/* Included as the actual prepared file_storage.c implementation after the
 * retained upstream notices/includes. No OS write fallback. */
#include "n64-save-files.h"
#include "n64-core-owner.h"
static void file_storage_save(void*,size_t,size_t);
int om_n64_file_storage_flush(struct file_storage*);
void om_n64_file_storage_discard_unstarted(struct file_storage*);

int open_file_storage(struct file_storage* storage,size_t size,const char* filename)
{
    int result;
    if(!om_n64_owner_access())return file_read_error;
    storage->data=NULL;storage->size=0;storage->offset=0;
    storage->filename=filename;storage->first_access=1;
    storage->write_blocked=1;storage->write_failed=0;storage->loaded_existing=0;
    if(!filename || !size || size>OM_N64_SAVE_MAX_BYTES || !om_n64_save_path(filename)){om_n64_owner_admission_failed();return file_size_error;}
    storage->data=calloc(1,size);
    if(!storage->data){om_n64_owner_admission_failed();return file_read_error;}
    storage->size=size;
    if(netplay_is_init())result=netplay_read_storage(filename,storage->data,size)==file_ok?OM_N64_SAVE_OK:OM_N64_SAVE_READ;
    else result=om_n64_save_read(filename,storage->data,size);
    if(result!=OM_N64_SAVE_OK && result!=OM_N64_SAVE_MISSING){
        free(storage->data);storage->data=NULL;storage->size=0;
        om_n64_owner_admission_failed();
        return result==OM_N64_SAVE_SIZE?file_size_error:file_read_error;
    }
    storage->loaded_existing=result==OM_N64_SAVE_OK;
    storage->write_blocked=0;return file_ok;
}

int open_rom_file_storage(struct file_storage* storage,const char* filename)
{
    file_status_t result;
    if(!om_n64_owner_access())return file_read_error;
    storage->data=NULL;storage->size=0;storage->offset=0;storage->filename=NULL;
    storage->first_access=1;storage->write_blocked=1;storage->write_failed=0;storage->loaded_existing=0;
    result=load_file(filename,(void**)&storage->data,&storage->size);
    if(result==file_ok)storage->filename=filename;
    return result;
}
int close_file_storage(struct file_storage* storage)
{
    if(!storage || !om_n64_owner_access())return 0;
    if(storage->data && !storage->write_blocked && om_n64_file_storage_flush(storage)!=1)return 0;
    om_n64_file_storage_discard_unstarted(storage);return 1;
}
void om_n64_file_storage_discard_unstarted(struct file_storage* storage)
{
    if(!storage || !om_n64_owner_access())return;
    free(storage->data);free((void*)storage->filename);
    storage->data=NULL;storage->filename=NULL;storage->size=0;storage->offset=0;
    storage->first_access=1;storage->write_blocked=1;storage->write_failed=0;storage->loaded_existing=0;
}
static uint8_t* file_storage_data(const void* opaque){return om_n64_owner_access()?((const struct file_storage*)opaque)->data:NULL;}
static size_t file_storage_size(const void* opaque){return om_n64_owner_access()?((const struct file_storage*)opaque)->size:0;}
static void file_storage_save(void* opaque,size_t start,size_t size)
{
    struct file_storage* storage=(struct file_storage*)opaque;
    int result;
    if(!storage || !om_n64_owner_access())return;
    if(netplay_is_init() && netplay_get_controller(0)==-1)return;
    if(storage->write_blocked || !storage->data || !storage->filename || start>storage->size || size>storage->size-start){storage->write_failed=1;return;}
    if(storage->first_access || storage->write_failed)
        result=om_n64_save_write(storage->filename,storage->data,storage->size);
    else result=om_n64_save_patch(storage->filename,storage->data+start,size,start);
    if(result==OM_N64_SAVE_OK){storage->first_access=0;storage->write_failed=0;return;}
    /* A failed chunk is retried as a full image on the next guest write. No
     * earlier dirty range is silently forgotten after a later successful chunk. */
    storage->write_failed=1;
    if(log_cb)log_cb(RETRO_LOG_WARN,"N64 storage publication failed (%d): %s\n",result,storage->filename);
}
int om_n64_file_storage_flush(struct file_storage* storage)
{
    if(!storage || !om_n64_owner_access() || storage->write_blocked || !storage->data || !storage->filename)return -1;
    if(netplay_is_init() && netplay_get_controller(0)==-1)return -1;
    storage->first_access=1;
    file_storage_save(storage,0,storage->size);
    return storage->write_failed?0:1;
}
static void file_storage_parent_save(void* opaque,size_t start,size_t size)
{
    if(!om_n64_owner_access())return;
    struct file_storage* child=(struct file_storage*)opaque;
    struct file_storage* parent=(struct file_storage*)child->filename;
    if(!parent || start>child->size || size>child->size-start || child->offset>parent->size || start>parent->size-child->offset || size>parent->size-child->offset-start){if(parent)parent->write_failed=1;return;}
    file_storage_save(parent,child->offset+start,size);
}
static void dummy_save(void* storage,size_t start,size_t size){(void)storage;(void)start;(void)size;}
const struct storage_backend_interface g_ifile_storage={file_storage_data,file_storage_size,file_storage_save};
const struct storage_backend_interface g_ifile_storage_ro={file_storage_data,file_storage_size,dummy_save};
const struct storage_backend_interface g_isubfile_storage={file_storage_data,file_storage_size,file_storage_parent_save};
