#pragma once
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Private, statically linked host API; not an IPC or renderer ABI. Calls are
// serialized internally and can block in the pinned upstream driver client.
// Run only inside the future owned helper, never on a browser/UI thread.
typedef struct VLKWinPadReport {
    int16_t sticks[4];
    uint16_t triggers[2];
    uint16_t buttons;
    uint16_t reserved; // Must be zero.
} VLKWinPadReport;

// No device operation: reserves one of four process-local slots and returns a
// private token. IDs must come from the admitted package's assigned HID identity,
// not a peer or a borrowed controller manufacturer. The first write must be zero.
// Zero return means success; other returns are Windows error codes.
uint32_t vlk_winpad_reserve(uint8_t slot, uint16_t vendor, uint16_t product,
                           uint64_t* token);
// Raw Windows GUID bytes (not UUID network order), available at reservation.
uint32_t vlk_winpad_container(uint64_t token, uint8_t container[16]);
// Read-only exact-container observation: 0 absent, 1 present, 2 unknown.
// Absence alone, without earlier presence/removal receipt, proves no cleanup.
uint32_t vlk_winpad_presence(const uint8_t container[16]);
uint32_t vlk_winpad_write(uint64_t token, const VLKWinPadReport* report);

// Always revokes further writes, best-effort neutralizes, closes the private
// driver connection, then checks the exact container in the present HID set.
// Success retires the token; repeated retire of that minted token is idempotent.
// Pending/unknown removal retains its slot for retry.
// A device created but never observed present cannot earn an absence receipt.
uint32_t vlk_winpad_retire(uint64_t token);

#ifdef __cplusplus
}
#endif
