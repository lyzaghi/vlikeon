"""Pinned PS1 source adaptations shared by native and Web recipes."""


def include_all_card_data(source):
    path=source/'mednafen/psx/frontio.c'
    raw=path.read_text(encoding='utf-8')
    old='''      if(self->data_used)
      {
         char tmp_name[64];'''
    new='''      /* OmniPlay: frontends can restore libretro SRAM directly without
       * touching data_used. A snapshot must retain that complete card too. */
      {
         char tmp_name[64];'''
    if raw.count(old)!=1:
        raise ValueError('The pinned PS1 memory-card snapshot anchor changed')
    path.write_text(raw.replace(old,new),encoding='utf-8',newline='\n')


def native_card_io(source, adapter):
    core=source/'libretro.c'
    raw=core.read_text(encoding='utf-8')
    old='void retro_run(void)\n{'
    if raw.count(old)!=1:
        raise ValueError('The pinned PS1 native run anchor changed')
    raw=raw.replace(old,'static void omniplay_upstream_run(void)\n{')
    core.write_text(raw+adapter.read_text(encoding='utf-8'),encoding='utf-8',newline='\n')
    exports=source/'link.T'
    raw=exports.read_text(encoding='utf-8')
    if raw.count('retro_*;')!=1:
        raise ValueError('The pinned PS1 export map changed')
    exports.write_text(raw.replace('retro_*;','retro_*;\n    omniplay_pause_guest;\n    omniplay_pause_guest_checked_v1;\n    omniplay_flush_io_checked;\n    omniplay_install_writable_files_v1;'),encoding='utf-8',newline='\n')
    apple=source/'libretro.osx.def'
    with apple.open('a',encoding='utf-8',newline='\n') as out:
        out.write('\n_omniplay_pause_guest\n_omniplay_pause_guest_checked_v1\n_omniplay_flush_io_checked\n_omniplay_install_writable_files_v1\n')


def native_memory_cards(source, adapters):
    """Route every pinned file-card producer/loader to an owned memory table.

    No arbitrary host stream is intercepted. BIOS/disc reads retain their
    existing readonly paths and the Windows owner withholds save/scratch writes.
    """
    writer=source/'mc_async.c'
    with writer.open('a',encoding='utf-8',newline='\n') as out:
        out.write('''\n/* Owner-thread query after the actual writer join. Not a save receipt. */
int om_ps1_card_writer_quiet(void) {
   unsigned i;
   if (mc_async_running || mc_async_thread || mc_async_slots_used > MC_ASYNC_SLOTS) return 0;
   for (i=0; i<mc_async_slots_used; ++i)
      if (retro_atomic_load_acquire_ptr(&mc_async_slots[i].pending) ||
          retro_atomic_load_acquire_int(&mc_async_slots[i].writing)) return 0;
   return 1;
}
''')
    header=source/'mc_async.h';raw=header.read_text(encoding='utf-8')
    anchor='void mc_async_flush_and_stop(void);'
    if raw.count(anchor)!=1:raise ValueError('The pinned PS1 writer barrier declaration changed')
    header.write_text(raw.replace(anchor,anchor+'\nint om_ps1_card_writer_quiet(void);'),encoding='utf-8',newline='\n')
    core=source/'libretro.c'
    raw=core.read_text(encoding='utf-8')
    include='#include "mc_async.h"'
    if raw.count(include)!=1:raise ValueError('The pinned PS1 card-writer include changed')
    raw=raw.replace(include,include+'\n#include "beetle-memory-files.h"')
    old='''   RFILE *mf = filestream_open(path,
         RETRO_VFS_FILE_ACCESS_WRITE, RETRO_VFS_FILE_ACCESS_HINT_NONE);
   if (!mf)
      return;
   filestream_write(mf, data, size);
   filestream_close(mf);'''
    if raw.count(old)!=1:raise ValueError('The pinned periodic PS1 card publisher changed')
    raw=raw.replace(old,'   (void)om_ps1_write_card(path,data,size);')
    swap='''   char memcard[MEMCARD_PATH_MAX];

   /* Save the slot's contents to the previously selected index */'''
    replacement='''   char memcard[MEMCARD_PATH_MAX];

   /* Join older queued writes before an inline swap can publish a newer card. */
   mc_async_flush_and_stop();
   om_ps1_card_write_begin();
   /* Save the slot's contents to the previously selected index */'''
    if raw.count(swap)!=1:raise ValueError('The pinned PS1 card-swap boundary changed')
    raw=raw.replace(swap,replacement)
    swap='   FrontIO_SaveMemcardToPath(PSX_FIO, port, memcard, true);'
    if raw.count(swap)!=1:raise ValueError('The pinned PS1 card-swap save changed')
    raw=raw.replace(swap,swap+'\n   if (om_ps1_card_write_failed()) {om_ps1_read_card(NULL,NULL,0);return;}')
    for before,after in [('bool retro_load_game(const struct retro_game_info *info)\n{','static bool omniplay_upstream_load_game(const struct retro_game_info *info)\n{'),('void retro_unload_game(void)\n{','static void omniplay_upstream_unload_game(void)\n{')]:
        if raw.count(before)!=1:raise ValueError('The pinned PS1 load/unload boundary changed')
        raw=raw.replace(before,after)
    core.write_text(raw,encoding='utf-8',newline='\n')
    frontend=source/'mednafen/psx/frontio.c'
    raw=frontend.read_text(encoding='utf-8')
    include='#include <compat/strl.h>'
    if raw.count(include)!=1:raise ValueError('The pinned PS1 front-I/O include changed')
    raw=raw.replace(include,include+'\n#include "../../beetle-memory-files.h"')
    adaptation=(adapters/'beetle-memory-frontio.c.inc').read_text(encoding='utf-8')
    marker='void FrontIO_SaveMemcardToPath('
    if adaptation.count(marker)!=1:raise ValueError('The PS1 memory card adaptation is ambiguous')
    load,save=adaptation.split(marker,1)
    load=load[load.index('void FrontIO_LoadMemcardFromPath('):]
    for begin,end,replacement in [('void FrontIO_LoadMemcardFromPath(FrontIO *self_, unsigned int which, const char *path, bool force_load)\n{','\nvoid FrontIO_SaveMemcard(',load),('void FrontIO_SaveMemcardToPath(FrontIO *self_, unsigned int which, const char *path, bool force_save)\n{','\nvoid FrontIO_SaveMemcardToPathAsync(',marker+save)]:
        if raw.count(begin)!=1:raise ValueError('The pinned PS1 file-card method changed')
        start=raw.index(begin);stop=raw.index(end,start)
        raw=raw[:start]+replacement.rstrip()+'\n'+raw[stop:]
    frontend.write_text(raw,encoding='utf-8',newline='\n')
    make=source/'Makefile.common';raw=make.read_text(encoding='utf-8')
    raw+='\n# OmniPlay bounded memory-card bridge, with no OS file writer.\nSOURCES_C += beetle-memory-files.c\n'
    make.write_text(raw,encoding='utf-8',newline='\n')
