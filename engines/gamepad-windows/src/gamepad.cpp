#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <setupapi.h>
#include <hidsdi.h>
#include <initguid.h>
#include <devpkey.h>
#include <objbase.h>
#include "WinUHid.h"
#include "VLKWinGamepad.h"
#include "hid_report.hpp"
#include <array>
#include <cwchar>
#include <cstring>
#include <limits>
#include <memory>
#include <mutex>
#include <new>

namespace {
// At most four live records, including unresolved cleanup. No actor-supplied
// pointers, arbitrary driver paths/IOCTLs, event callbacks or hardware IDs.
struct Device {
    std::uint64_t token=0;
    std::uint16_t vendor=0,product=0;
    GUID container{};
    PWINUHID_DEVICE handle=nullptr;
    bool attempted=false,ever_present=false,closing=false;
};
std::mutex mutex;
std::array<std::unique_ptr<Device>,4> devices;
std::uint64_t next_token=1;

DWORD error_or(DWORD fallback) noexcept {
    const auto error=GetLastError();return error==ERROR_SUCCESS ? fallback : error;
}
Device* find(std::uint64_t token) noexcept {
    if (token==0) return nullptr;
    for (auto& device:devices) if (device && device->token==token) return device.get();
    return nullptr;
}
enum class Presence { Present,Absent,Unknown };
Presence presence(const GUID& container) noexcept {
    GUID hid{};HidD_GetHidGuid(&hid);
    const auto list=SetupDiGetClassDevsW(&hid,nullptr,nullptr,
                                       DIGCF_DEVICEINTERFACE|DIGCF_PRESENT);
    if (list==INVALID_HANDLE_VALUE) return Presence::Unknown;
    bool unknown=false;
    Presence result=Presence::Unknown;
    // A failed property/enumeration or unexpectedly large inventory is not an
    // absence receipt. No device removal, rescan or mutation is requested here.
    for (DWORD index=0;index<4096;++index) {
        SP_DEVINFO_DATA data{};data.cbSize=sizeof(data);
        if (!SetupDiEnumDeviceInfo(list,index,&data)) {
            if (GetLastError()==ERROR_NO_MORE_ITEMS && !unknown) result=Presence::Absent;
            break;
        }
        GUID actual{};DEVPROPTYPE type=0;DWORD required=0;
        if (!SetupDiGetDevicePropertyW(list,&data,&DEVPKEY_Device_ContainerId,
              &type,reinterpret_cast<PBYTE>(&actual),sizeof(actual),&required,0) ||
            type!=DEVPROP_TYPE_GUID || required!=sizeof(actual)) {
            unknown=true;continue;
        }
        if (IsEqualGUID(actual,container)) {result=Presence::Present;break;}
    }
    SetupDiDestroyDeviceInfoList(list);return result;
}
DWORD retire(Device& device) noexcept {
    device.closing=true;
    // A partial start may become visible only after initialize has failed.
    // Observe that identity before closing, if possible; never infer it from
    // absence. This can also establish late presence on a later cleanup retry.
    if (device.attempted && !device.ever_present &&
        presence(device.container)==Presence::Present) device.ever_present=true;
    if (device.handle) {
        const auto zero=vlikeon::winpad::encode(VLKWinPadReport{});
        // Failure to neutralize does not suppress connection teardown. These
        // upstream calls are synchronous and have no bounded completion promise.
        WinUHidSubmitInputReport(device.handle,zero.data(),static_cast<DWORD>(zero.size()));
        WinUHidDestroyDevice(device.handle);device.handle=nullptr;
    }
    if (!device.attempted) return ERROR_SUCCESS; // Only an in-memory reservation.
    // Never equate a failed/unfinished enumeration with removal. When the device
    // never became visible, retain the record until the helper is retired; later
    // absence alone cannot disprove delayed enumeration after a failed create.
    if (!device.ever_present) return ERROR_IO_INCOMPLETE;
    return presence(device.container)==Presence::Absent ? ERROR_SUCCESS : ERROR_IO_INCOMPLETE;
}
DWORD initialize(Device& device) noexcept {
    if (WinUHidGetDriverInterfaceVersion()!=1) return ERROR_REVISION_MISMATCH;
    wchar_t instance[39]{};
    if (StringFromGUID2(device.container,instance,39)!=39) return ERROR_GEN_FAILURE;
    WINUHID_DEVICE_CONFIG config{};
    config.SupportedEvents=WINUHID_EVENT_NONE;
    config.VendorID=device.vendor;config.ProductID=device.product;config.VersionNumber=1;
    config.ContainerId=device.container;config.InstanceID=instance;
    config.ReportDescriptor=vlikeon::winpad::descriptor;
    config.ReportDescriptorLength=static_cast<USHORT>(sizeof(vlikeon::winpad::descriptor));
    device.attempted=true;
    device.handle=WinUHidCreateDevice(&config);
    if (!device.handle) return error_or(ERROR_GEN_FAILURE);
    const auto zero=vlikeon::winpad::encode(VLKWinPadReport{});
    if (!WinUHidSubmitInputReport(device.handle,zero.data(),static_cast<DWORD>(zero.size())))
        return error_or(ERROR_WRITE_FAULT);
    if (!WinUHidStartDevice(device.handle,nullptr,nullptr)) return error_or(ERROR_NOT_READY);
    // A finite readiness window for our enumeration loop only. Underlying
    // WinUHid/SetupAPI calls can still block: helper isolation is mandatory.
    const auto deadline=GetTickCount64()+1000;
    do {
        const auto state=presence(device.container);
        if (state==Presence::Present) {device.ever_present=true;return ERROR_SUCCESS;}
        if (state==Presence::Unknown) return ERROR_NOT_READY;
        Sleep(10);
    } while (GetTickCount64()<deadline);
    return ERROR_TIMEOUT;
}
}

extern "C" std::uint32_t vlk_winpad_reserve(std::uint8_t slot,std::uint16_t vendor,
                                           std::uint16_t product,std::uint64_t* token) {
    if (!token) return ERROR_INVALID_PARAMETER;
    *token=0;
    if (slot>=devices.size() || vendor==0 || product==0) return ERROR_INVALID_PARAMETER;
    try {
        std::lock_guard lock(mutex);
        // Drop can leave a bounded tombstone. A later reservation retries its
        // real cleanup but can never steal a still-active or unconfirmed slot.
        if (devices[slot] && devices[slot]->closing && retire(*devices[slot])==ERROR_SUCCESS)
            devices[slot].reset();
        if (devices[slot]) return ERROR_BUSY;
        if (next_token==std::numeric_limits<std::uint64_t>::max()) return ERROR_TOO_MANY_CMDS;
        auto device=std::make_unique<Device>();
        if (FAILED(CoCreateGuid(&device->container))) return ERROR_GEN_FAILURE;
        device->token=next_token++;device->vendor=vendor;device->product=product;
        *token=device->token;devices[slot]=std::move(device);return ERROR_SUCCESS;
    } catch (const std::bad_alloc&) {return ERROR_NOT_ENOUGH_MEMORY;}
      catch (...) {return ERROR_GEN_FAILURE;}
}
extern "C" std::uint32_t vlk_winpad_container(std::uint64_t token,std::uint8_t container[16]) {
    if (!container) return ERROR_INVALID_PARAMETER;
    std::memset(container,0,16);
    try {
        std::lock_guard lock(mutex);
        const auto* device=find(token);
        if (!device || device->closing) return ERROR_INVALID_HANDLE;
        static_assert(sizeof(device->container)==16);
        std::memcpy(container,&device->container,16);return ERROR_SUCCESS;
    } catch (...) {return ERROR_GEN_FAILURE;}
}
extern "C" std::uint32_t vlk_winpad_presence(const std::uint8_t container[16]) {
    if (!container) return 2;
    GUID id{};static_assert(sizeof(id)==16);std::memcpy(&id,container,16);
    const GUID zero{};if (IsEqualGUID(id,zero)) return 2;
    switch (presence(id)) {
    case Presence::Absent:return 0;
    case Presence::Present:return 1;
    default:return 2;
    }
}
extern "C" std::uint32_t vlk_winpad_write(std::uint64_t token,const VLKWinPadReport* report) {
    if (!report || report->reserved!=0) return ERROR_INVALID_PARAMETER;
    try {
        std::lock_guard lock(mutex);
        auto* device=find(token);
        if (!device || device->closing) return ERROR_INVALID_HANDLE;
        if (!device->attempted) {
            if (!vlikeon::winpad::neutral(*report)) return ERROR_INVALID_STATE;
            const auto error=initialize(*device);
            if (error!=ERROR_SUCCESS) {device->closing=true;return error;}
        }
        if (!device->handle) return ERROR_INVALID_HANDLE;
        const auto bytes=vlikeon::winpad::encode(*report);
        if (WinUHidSubmitInputReport(device->handle,bytes.data(),static_cast<DWORD>(bytes.size())))
            return ERROR_SUCCESS;
        const auto error=error_or(ERROR_WRITE_FAULT);device->closing=true;return error;
    } catch (...) {return ERROR_GEN_FAILURE;}
}
extern "C" std::uint32_t vlk_winpad_retire(std::uint64_t token) {
    try {
        std::lock_guard lock(mutex);
        auto* device=find(token);
        // Another local provider instance may have retried a closing registry
        // record and confirmed removal. Tokens are monotonic and never reused:
        // an already minted token absent from the registry is a stored removal
        // receipt, not authority to retire the replacement occupying that slot.
        if (!device) return token>0 && token<next_token ? ERROR_SUCCESS : ERROR_INVALID_HANDLE;
        const auto error=retire(*device);
        if (error==ERROR_SUCCESS)
            for (auto& slot:devices) if (slot.get()==device) {slot.reset();break;}
        return error;
    } catch (...) {return ERROR_GEN_FAILURE;}
}
