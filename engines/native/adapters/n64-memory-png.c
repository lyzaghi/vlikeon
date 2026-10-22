/* Bounded scratch PNG writer using the exact selected native libpng/zlib. */
#include "n64-memory-png.h"
#include <png.h>
#include <setjmp.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#if !defined(PNG_USER_MEM_SUPPORTED) || !defined(PNG_SETJMP_SUPPORTED) || !defined(PNG_WRITE_SUPPORTED)
#error The selected codec must support custom allocation, setjmp and PNG write
#endif

typedef union OmPngAllocation OmPngAllocation;
typedef struct OmPngState {
    OmniN64File* file;
    png_structp png;
    png_infop info;
    size_t live,limit;
    OmPngAllocation* allocations;
} OmPngState;
union OmPngAllocation {
    struct {size_t charge;OmPngAllocation* previous;OmPngAllocation* next;} item;
    long double align_number;
    void* align_pointer;
    max_align_t align_standard;
};

static png_voidp PNGCBAPI om_png_allocate(png_structp png,png_alloc_size_t count)
{
    OmPngState* state=(OmPngState*)png_get_mem_ptr(png);
    OmPngAllocation* allocation;
    size_t charge;
    if (!state || !count || count>SIZE_MAX-sizeof(OmPngAllocation)) return NULL;
    charge=(size_t)count+sizeof(OmPngAllocation);
    if (state->live>state->limit || charge>state->limit-state->live) return NULL;
    allocation=(OmPngAllocation*)malloc(charge);
    if (!allocation) return NULL;
    allocation->item.charge=charge;allocation->item.previous=NULL;
    allocation->item.next=state->allocations;
    if (state->allocations) state->allocations->item.previous=allocation;
    state->allocations=allocation;state->live+=charge;
    return (png_voidp)(allocation+1);
}
static void PNGCBAPI om_png_free(png_structp png,png_voidp data)
{
    OmPngState* state=(OmPngState*)png_get_mem_ptr(png);
    OmPngAllocation* allocation;
    if (!data || !state) return;
    allocation=((OmPngAllocation*)data)-1;
    if (allocation->item.previous) allocation->item.previous->item.next=allocation->item.next;
    else state->allocations=allocation->item.next;
    if (allocation->item.next) allocation->item.next->item.previous=allocation->item.previous;
    state->live-=allocation->item.charge;free(allocation);
}
static void PNGCBAPI om_png_error(png_structp png,png_const_charp message)
{
    (void)message;png_longjmp(png,1);
}
static void PNGCBAPI om_png_warning(png_structp png,png_const_charp message)
{
    (void)png;(void)message;
}
static void PNGCBAPI om_png_write(png_structp png,png_bytep bytes,png_size_t count)
{
    OmPngState* state=(OmPngState*)png_get_io_ptr(png);
    if (!state || om_n64_fwrite(bytes,1,count,state->file)!=count) png_error(png,"scratch PNG write failed");
}
static void PNGCBAPI om_png_flush(png_structp png)
{
    OmPngState* state=(OmPngState*)png_get_io_ptr(png);
    if (!state || om_n64_fflush(state->file)!=0) png_error(png,"scratch PNG flush failed");
}
static void om_png_destroy(OmPngState* state)
{
    if (state->png) png_destroy_write_struct(&state->png,&state->info);
    /* No codec producer survives destruction. Retain an allocation list so an
     * aborted/error path cannot accumulate unowned buffers across calls. */
    while (state->allocations) {
        OmPngAllocation* allocation=state->allocations;
        state->allocations=allocation->item.next;free(allocation);
    }
    free(state);
}

int om_n64_png_encode(OmniN64File* file,const unsigned char* source,uint64_t source_bytes,
                     uint32_t width,uint32_t height,uint32_t stride,
                     uint32_t channels,uint32_t flip,uint32_t codec_limit)
{
    OmPngState* state;
    uint64_t row_bytes,required;
    uint32_t row;
    png_color_8 significant;
    if (!file) return 0;
    row_bytes=(uint64_t)width*channels;
    if (!source || !width || !height || width>16384 || height>16384 ||
        (channels!=3 && channels!=4) || flip>1 || stride<row_bytes ||
        !codec_limit || codec_limit>OM_N64_PNG_CODEC_LIMIT || source_bytes>256ull*1024*1024) {
        om_n64_abort(file);return 0;
    }
    required=(uint64_t)(height-1)*stride+row_bytes;
    if (required>source_bytes || required>256ull*1024*1024 || required>SIZE_MAX) {om_n64_abort(file);return 0;}
    state=(OmPngState*)calloc(1,sizeof(*state));
    if (!state) {om_n64_abort(file);return 0;}
    state->file=file;state->limit=codec_limit;
    state->png=png_create_write_struct_2(PNG_LIBPNG_VER_STRING,state,om_png_error,om_png_warning,state,om_png_allocate,om_png_free);
    if (!state->png) {om_n64_abort(file);om_png_destroy(state);return 0;}
    /* State and codec-owned pointers live in heap storage, so changes after
     * setjmp remain defined after longjmp; no C++ destructor is crossed. */
    if (setjmp(png_jmpbuf(state->png))) {om_n64_abort(file);om_png_destroy(state);return 0;}
    state->info=png_create_info_struct(state->png);
    if (!state->info) {om_n64_abort(file);om_png_destroy(state);return 0;}
    png_set_write_fn(state->png,state,om_png_write,om_png_flush);
    significant.red=significant.green=significant.blue=8;significant.alpha=8;significant.gray=8;
    png_set_sBIT(state->png,state->info,&significant);
    png_set_IHDR(state->png,state->info,width,height,8,channels==4?PNG_COLOR_TYPE_RGB_ALPHA:PNG_COLOR_TYPE_RGB,
        PNG_INTERLACE_NONE,PNG_COMPRESSION_TYPE_DEFAULT,PNG_FILTER_TYPE_DEFAULT);
    png_write_info(state->png,state->info);
    for (row=0;row<height;++row) {
        const uint64_t index=flip?(uint64_t)(height-1-row):row;
        png_write_row(state->png,(png_const_bytep)(source+index*stride));
    }
    png_write_end(state->png,state->info);
    if (om_n64_fflush(file)!=0) {om_n64_abort(file);om_png_destroy(state);return 0;}
    om_png_destroy(state);return 1;
}
