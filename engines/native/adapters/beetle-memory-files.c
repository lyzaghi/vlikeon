/* Native PS1 card bridge. Game/BIOS/disc inputs retain their readonly stagers;
 * every admitted card write uses the bounded frontend-owned memory table. */
#include "beetle-memory-files.h"
#include <retro_atomic.h>
#include <stdlib.h>
#include <string.h>

#define OM_PS1_CARD_BYTES (1u<<17)
#define OM_PS1_PATH_BYTES 512
static struct OmniWritableFilesV1 om_ps1_files;
static int om_ps1_files_ready;
static retro_atomic_int_t om_ps1_read_failure;
static retro_atomic_int_t om_ps1_write_failure;

int om_ps1_files_install(const struct OmniWritableFilesV1* value)
{
   if (!value)
   {
      memset(&om_ps1_files,0,sizeof(om_ps1_files));
      om_ps1_files_ready=0;
      retro_atomic_store_release(&om_ps1_read_failure,0);
      retro_atomic_store_release(&om_ps1_write_failure,0);
      return 1;
   }
   if (value->version!=1 || value->struct_bytes!=sizeof(*value) || !value->context ||
       !value->stat || !value->mkdir || !value->open || !value->close ||
       !value->read || !value->write || !value->resize || !value->flush ||
       !value->remove || !value->rename || !value->list || !value->usage)
      return 0;
   om_ps1_files=*value;
   om_ps1_files_ready=1;
   retro_atomic_store_release(&om_ps1_read_failure,0);
   retro_atomic_store_release(&om_ps1_write_failure,0);
   return 1;
}
int om_ps1_files_installed(void) {return om_ps1_files_ready;}

static int om_ps1_path(const char* raw,char path[OM_PS1_PATH_BYTES])
{
   size_t at=0;
   if (!raw) return 0;
   while (at<OM_PS1_PATH_BYTES && raw[at])
   {
      path[at]=raw[at]=='\\'?'/':raw[at];
      ++at;
   }
   if (at<10 || at>=OM_PS1_PATH_BYTES) return 0;
   path[at]=0;
   if (memcmp(path,"save/PSX/",9) ||
       strspn(path+9,"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-")!=at-9 ||
       strstr(path+9,"..") || !strchr(path+9,'.') || at<4 || strcmp(path+at-4,".mcr"))
      return 0;
   return 1;
}

static int om_ps1_failed_read(void)
{
   retro_atomic_store_release(&om_ps1_read_failure,1);
   return -1;
}
int om_ps1_card_read_failed(void) {return retro_atomic_load_acquire(&om_ps1_read_failure)!=0;}
void om_ps1_card_read_begin(void) {retro_atomic_store_release(&om_ps1_read_failure,0);}
int om_ps1_card_write_failed(void) {return retro_atomic_load_acquire(&om_ps1_write_failure)!=0;}
void om_ps1_card_write_begin(void) {retro_atomic_store_release(&om_ps1_write_failure,0);}
static int om_ps1_failed_write(void) {retro_atomic_store_release(&om_ps1_write_failure,1);return 0;}

int om_ps1_read_card(const char* raw,uint8_t* output,uint32_t bytes)
{
   char path[OM_PS1_PATH_BYTES];struct OmniFilesStatV1 shape;
   uint64_t handle=0,actual=0;uint8_t* staged;int result,closed;
   if (!om_ps1_files_ready || !output || bytes!=OM_PS1_CARD_BYTES || !om_ps1_path(raw,path))
      return om_ps1_failed_read();
   memset(&shape,0,sizeof(shape));
   result=om_ps1_files.stat(om_ps1_files.context,path,&shape);
   if (result==OMNI_FILES_NOT_FOUND) return 0;
   if (result || shape.kind!=OMNI_FILES_REGULAR || shape.bytes!=bytes)
      return om_ps1_failed_read();
   staged=(uint8_t*)malloc(bytes);
   if (!staged) return om_ps1_failed_read();
   result=om_ps1_files.open(om_ps1_files.context,path,OMNI_FILES_READ,&handle);
   if (result) {free(staged);return om_ps1_failed_read();}
   result=om_ps1_files.read(om_ps1_files.context,handle,0,staged,bytes,&actual);
   closed=om_ps1_files.close(om_ps1_files.context,handle);
   if (result || actual!=bytes || closed)
   {free(staged);return om_ps1_failed_read();}
   memcpy(output,staged,bytes);free(staged);return 1;
}

int om_ps1_write_card(const char* raw,const uint8_t* input,uint32_t bytes)
{
   char path[OM_PS1_PATH_BYTES],temporary[OM_PS1_PATH_BYTES+8];
   size_t length;uint64_t handle=0,actual=0;int result,written,flushed,closed,published;
   if (!om_ps1_files_ready || om_ps1_card_read_failed() || !input || bytes!=OM_PS1_CARD_BYTES || !om_ps1_path(raw,path))
      return om_ps1_failed_write();
   length=strlen(path);memcpy(temporary,path,length);memcpy(temporary+length,".omtmp",7);
   result=om_ps1_files.remove(om_ps1_files.context,temporary);
   if (result!=OMNI_FILES_OK && result!=OMNI_FILES_NOT_FOUND) return om_ps1_failed_write();
   result=om_ps1_files.open(om_ps1_files.context,temporary,OMNI_FILES_WRITE|OMNI_FILES_CREATE|OMNI_FILES_EXCLUSIVE,&handle);
   if (result) return om_ps1_failed_write();
   written=om_ps1_files.write(om_ps1_files.context,handle,0,input,bytes,&actual)==OMNI_FILES_OK && actual==bytes;
   flushed=om_ps1_files.flush(om_ps1_files.context,handle)==OMNI_FILES_OK;
   closed=om_ps1_files.close(om_ps1_files.context,handle)==OMNI_FILES_OK;
   published=written && flushed && closed && om_ps1_files.rename(om_ps1_files.context,temporary,path,1)==OMNI_FILES_OK;
   if (!published) om_ps1_files.remove(om_ps1_files.context,temporary);
   return published?1:om_ps1_failed_write();
}
