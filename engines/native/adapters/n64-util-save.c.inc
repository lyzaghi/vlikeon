/* Actual selected utility implementation: bounded readonly host assets and
 * separate persistent save callbacks. No host write mode is present. */
#include "n64-save-files.h"
static file_status_t omniplay_readonly_extent(FILE* file,size_t* bytes)
{
    long value;
    if(fseek(file,0,SEEK_END)!=0)return file_size_error;
    value=ftell(file);
    if(value<0 || (unsigned long)value>OM_N64_SAVE_MAX_BYTES || fseek(file,0,SEEK_SET)!=0)return file_size_error;
    *bytes=(size_t)value;return file_ok;
}
static file_status_t omniplay_readonly_read_from_file(const char* filename,void* output,size_t expected)
{
    FILE* file;void* candidate;size_t bytes=0;file_status_t result;int good,closed;
    if(!filename || (!output && expected) || expected>OM_N64_SAVE_MAX_BYTES)return file_size_error;
    file=fopen(filename,"rb");if(!file)return file_open_error;
    result=omniplay_readonly_extent(file,&bytes);
    if(result!=file_ok || bytes!=expected){fclose(file);return file_size_error;}
    candidate=malloc(bytes?bytes:1);
    if(!candidate){fclose(file);return file_read_error;}
    good=fread(candidate,1,bytes,file)==bytes && fgetc(file)==EOF && !ferror(file);closed=fclose(file)==0;
    if(!good || !closed){free(candidate);return file_read_error;}
    if(bytes)memcpy(output,candidate,bytes);free(candidate);return file_ok;
}
static file_status_t omniplay_readonly_load_file(const char* filename,void** output,size_t* length)
{
    FILE* file;void* candidate;size_t bytes=0;file_status_t result;int good,closed;
    if(!filename || !output || !length)return file_size_error;
    file=fopen(filename,"rb");if(!file)return file_open_error;
    result=omniplay_readonly_extent(file,&bytes);
    if(result!=file_ok){fclose(file);return result;}
    candidate=malloc(bytes?bytes:1);if(!candidate){fclose(file);return file_read_error;}
    good=fread(candidate,1,bytes,file)==bytes && fgetc(file)==EOF && !ferror(file);closed=fclose(file)==0;
    if(!good || !closed){free(candidate);return file_read_error;}
    *output=candidate;*length=bytes;return file_ok;
}
static file_status_t omniplay_readonly_get_file_size(const char* filename,size_t* length)
{
    FILE* file;size_t bytes=0;file_status_t result;int closed;
    if(!filename || !length)return file_size_error;
    file=fopen(filename,"rb");if(!file)return file_open_error;
    result=omniplay_readonly_extent(file,&bytes);closed=fclose(file)==0;
    if(result!=file_ok)return result;if(!closed)return file_read_error;
    *length=bytes;return file_ok;
}
static file_status_t omniplay_save_status(int result)
{
    switch(result){
    case OM_N64_SAVE_OK:return file_ok;
    case OM_N64_SAVE_MISSING:case OM_N64_SAVE_OPEN:return file_open_error;
    case OM_N64_SAVE_WRITE:return file_write_error;
    case OM_N64_SAVE_SIZE:return file_size_error;
    default:return file_read_error;
    }
}
file_status_t read_from_file(const char* filename,void* data,size_t size)
{
    if(om_n64_save_path(filename))return omniplay_save_status(om_n64_save_read(filename,data,size));
    return omniplay_readonly_read_from_file(filename,data,size);
}
file_status_t write_to_file(const char* filename,const void* data,size_t size)
{
    return omniplay_save_status(om_n64_save_write(filename,data,size));
}
file_status_t write_chunk_to_file(const char* filename,const void* data,size_t size,size_t offset)
{
    return omniplay_save_status(om_n64_save_patch(filename,data,size,offset));
}
file_status_t load_file(const char* filename,void** buffer,size_t* size)
{
    if(om_n64_save_path(filename))return omniplay_save_status(om_n64_save_load(filename,buffer,size));
    return omniplay_readonly_load_file(filename,buffer,size);
}
file_status_t get_file_size(const char* filename,size_t* size)
{
    if(om_n64_save_path(filename))return omniplay_save_status(om_n64_save_size(filename,size));
    return omniplay_readonly_get_file_size(filename,size);
}
