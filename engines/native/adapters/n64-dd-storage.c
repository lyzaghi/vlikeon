#include "n64-dd-storage.h"
#include "n64-save-files.h"
#include "n64-core-owner.h"
#include "n64-snapshot-bounds.h"
#include "backends/file_storage.h"
#include "backends/api/storage_backend.h"
#include "device/dd/disk.h"
#include "device/dd/dd_controller.h"
#include "api/m64p_types.h"
#include "main/util.h"
#include <md5.h>
#include <errno.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

struct OmDdLayout {size_t bytes,system,id,ram,ram_bytes;unsigned int format,development;};
/* At most old/new descriptors during a serialized media swap. The actual
 * selected core uses one. No record is dereferenced by the quiescence query. */
static struct dd_disk* live_disks[2];
static unsigned int owned_slot(const struct dd_disk* disk)
{unsigned int i;for(i=0;i<2;++i)if(live_disks[i]==disk)return i;return 2;}
int om_n64_dd_snapshot_id(const struct dd_disk* disk,uint32_t* output)
{
    const uint8_t* data;size_t bytes;
    if(!om_n64_owner_access() || !disk || !output || owned_slot(disk)==2 || !disk->storage || !disk->istorage || !disk->istorage->data || !disk->istorage->size)return 0;
    bytes=disk->istorage->size(disk->storage);
    data=disk->istorage->data(disk->storage);
    return om_n64_snapshot_disk_word(data,bytes,disk->offset_id,output);
}
int om_n64_dd_quiescent(void){return om_n64_owner_access() && !live_disks[0] && !live_disks[1];}
static int parse(uint8_t** data,struct OmDdLayout* layout)
{
    uint8_t* candidate=scan_and_expand_disk_format(*data,&layout->bytes,&layout->format,&layout->development,&layout->system,&layout->id,&layout->ram,&layout->ram_bytes);
    if(!candidate)return 0;
    *data=candidate;return 1;
}
static char* legacy_name(const char* source,int mode)
{
    size_t bytes=0;const char* suffix=mode==1?".ram":".disk_save";
    while(bytes<=1024 && source[bytes])++bytes;
    if(!bytes || bytes>1024 || strlen(suffix)>1024-bytes)return NULL;
    return formatstr("%s%s",source,suffix);
}
int om_n64_dd_open(struct dd_disk* output,const struct storage_backend_interface** output_interface,const char* source,int mode)
{
    struct dd_disk staged;
    struct OmDdLayout base={0},saved={0};
    struct file_storage* owner=NULL;struct file_storage* writable=NULL;
    uint8_t* data=NULL;void* retained=NULL;size_t retained_bytes=0;
    char* destination=NULL;char* legacy=NULL;
    md5_state_t md5;md5_byte_t digest[16];char hex[33];unsigned int i;
    const struct storage_backend_interface* selected=NULL;
    int result,has_save=0,imported=0;unsigned int slot;
    if(!om_n64_owner_access() || !output || !output_interface || !source || !source[0] || output->storage || output->save_storage || mode < -1 || mode>1)return 0;
    if(owned_slot(output)<2)return 0;
    for(slot=0;slot<2 && live_disks[slot];++slot){}
    if(slot==2)return 0;
    memset(&staged,0,sizeof(staged));
    if(load_file(source,(void**)&data,&base.bytes)!=file_ok || !data || base.bytes>OM_N64_SAVE_MAX_BYTES)goto failed;
    md5_init(&md5);md5_append(&md5,(const md5_byte_t*)data,(int)base.bytes);md5_finish(&md5,digest);
    for(i=0;i<16;++i){static const char digits[]="0123456789abcdef";hex[i*2]=digits[digest[i]>>4];hex[i*2+1]=digits[digest[i]&15];}hex[32]=0;
    if(!parse(&data,&base))goto failed;
    if(base.format==DISK_FORMAT_MAME && mode==1)mode=0;
    if(mode>=0){
        destination=formatstr("save/N64/dd/%s%s",hex,mode==1?".ram":".disk_save");
        if(!destination)goto failed;
        for(i=0;i<2;++i)if(live_disks[i] && live_disks[i]->save_storage){
            const struct file_storage* prior=(const struct file_storage*)live_disks[i]->save_storage;
            if(prior->filename && !strcmp(prior->filename,destination))goto failed;
        }
        result=om_n64_save_load(destination,&retained,&retained_bytes);
        if(result==OM_N64_SAVE_OK)has_save=1;
        else if(result!=OM_N64_SAVE_MISSING)goto failed;
        else{
            file_status_t status;
            legacy=legacy_name(source,mode);if(!legacy)goto failed;
            errno=0;status=load_file(legacy,&retained,&retained_bytes);
            if(status==file_ok){has_save=1;imported=1;}
            else if(status!=file_open_error || errno!=ENOENT)goto failed;
        }
        if(has_save && mode==0){
            uint8_t* incoming=(uint8_t*)retained;retained=NULL;saved.bytes=retained_bytes;
            if(!parse(&incoming,&saved)){free(incoming);goto failed;}
            /* A full image remains the original physical representation.
             * Guest reformatting may change valid system/ID data; do not
             * invent an immutable user-data fingerprint for those fields. */
            if(saved.bytes!=base.bytes || saved.format!=base.format){free(incoming);goto failed;}
            free(data);data=incoming;base=saved;
        }else if(has_save){
            if(retained_bytes!=base.ram_bytes || base.ram>base.bytes || base.ram_bytes>base.bytes-base.ram)goto failed;
            if(retained_bytes)memcpy(data+base.ram,retained,retained_bytes);
            free(retained);retained=NULL;
        }
    }
    owner=(struct file_storage*)calloc(1,sizeof(*owner));if(!owner)goto failed;
    owner->data=data;data=NULL;owner->size=base.bytes;owner->filename=destination;destination=NULL;owner->write_blocked=1;
    staged.storage=owner;staged.istorage=&g_ifile_storage_ro;
    staged.format=(uint8_t)base.format;staged.development=(uint8_t)base.development;
    staged.offset_sys=base.system;staged.offset_id=base.id;staged.offset_ram=base.ram;
    if(mode>=0){
        writable=(struct file_storage*)calloc(1,sizeof(*writable));if(!writable)goto failed;
        writable->data=owner->data+(mode==1?base.ram:0);writable->size=mode==1?base.ram_bytes:base.bytes;
        writable->filename=owner->filename;writable->first_access=!has_save;writable->loaded_existing=has_save;
        staged.save_storage=writable;staged.isave_storage=&g_ifile_storage;
    }
    selected=mode<0?&g_istorage_disk_read_only:mode==1?&g_istorage_disk_ram_only:&g_istorage_disk_full;
    if(!GenerateLBAToPhysTable(&staged))goto failed;
    {
        const struct dd_sys_data* sys=(const struct dd_sys_data*)(owner->data+base.system);
        uint32_t region=big32(sys->region);
        staged.region=base.development || region==DD_REGION_DV?DDREGION_DEV:region==DD_REGION_JP?DDREGION_JAPAN:region==DD_REGION_US?DDREGION_US:DDREGION_UNKNOWN;
    }
    /* This is the last fallible publication step. Legacy media remain readonly
     * and an unsuccessful memory commit cannot masquerade as an imported save. */
    if(imported){
        result=om_n64_save_write(writable->filename,writable->data,writable->size);
        if(result!=OM_N64_SAVE_OK)goto failed;
        writable->first_access=0;
    }
    free(legacy);*output=staged;*output_interface=selected;live_disks[slot]=output;return 1;
failed:
    free(legacy);free(destination);free(retained);free(data);free(writable);
    if(owner)close_file_storage(owner);free(owner);
    return 0;
}
int om_n64_dd_flush(struct dd_disk* disk)
{
    struct file_storage* storage;
    if(!om_n64_owner_access() || !disk || owned_slot(disk)>=2 || !disk->storage || !disk->istorage)return -1;
    if(!disk->save_storage)return disk->isave_storage?-1:1;
    if(disk->isave_storage!=&g_ifile_storage)return -1;
    storage=(struct file_storage*)disk->save_storage;
    storage->first_access=1;
    g_ifile_storage.save(storage,0,storage->size);
    return storage->write_blocked?-1:storage->write_failed?0:1;
}
int om_n64_dd_close(struct dd_disk* disk)
{
    int complete;
    if(!disk || !om_n64_owner_access())return 0;
    if(!disk->storage && !disk->save_storage){if(owned_slot(disk)<2)return 0;memset(disk,0,sizeof(*disk));return 1;}
    complete=om_n64_dd_flush(disk)==1;
    if(!complete)return 0; /* Keep data, descriptor and ownership for retry. */
    om_n64_dd_discard_unstarted(disk);return 1;
}
int om_n64_dd_flush_all(void)
{
    unsigned int i;int complete=1;
    if(!om_n64_owner_access())return -1;
    for(i=0;i<2;++i)if(live_disks[i]){int result=om_n64_dd_flush(live_disks[i]);if(result<0)return -1;if(!result)complete=0;}
    return complete;
}
int om_n64_dd_retire_all(void)
{
    unsigned int i;int complete=1;
    if(!om_n64_owner_access())return 0;
    for(i=0;i<2;++i)if(live_disks[i] && !om_n64_dd_close(live_disks[i]))complete=0;
    return complete;
}
void om_n64_dd_discard_unstarted(struct dd_disk* disk)
{
    unsigned int slot;
    if(!disk || !om_n64_owner_access())return;slot=owned_slot(disk);if(slot>=2)return;
    free(disk->save_storage);
    if(disk->storage){close_file_storage((struct file_storage*)disk->storage);free(disk->storage);}
    memset(disk,0,sizeof(*disk));live_disks[slot]=NULL;
}
