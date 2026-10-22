
/* Native-only pinned PS1 adaptation. retro_run owns card production; pause
 * joins the upstream bounded asynchronous card writer. Immutable CD read-ahead
 * may continue, but cannot produce card writes. Unload joins that reader too. */
#ifdef HAVE_LIGHTREC
#error OmniPlay PS1 native savedata requires the interpreter profile
#endif
static int om_ps1_core_loaded;
RETRO_API bool retro_load_game(const struct retro_game_info *info)
{
   bool loaded;
   if (om_ps1_core_loaded || !om_ps1_files_installed()) return false;
   om_ps1_card_read_begin();
   loaded=omniplay_upstream_load_game(info);
   if (loaded && om_ps1_card_read_failed())
   {omniplay_upstream_unload_game();return false;}
   om_ps1_core_loaded=loaded;
   return loaded;
}
RETRO_API void retro_unload_game(void)
{
   omniplay_upstream_unload_game();
   om_ps1_core_loaded=0;
}
RETRO_API int omniplay_install_writable_files_v1(const struct OmniWritableFilesV1* value)
{
   if (om_ps1_core_loaded) return 0;
   mc_async_flush_and_stop();
   return om_ps1_files_install(value);
}
RETRO_API void retro_run(void)
{
   /* A pause/capture may have joined the writer. Restart lazily only when the
    * frontend actually runs another guest frame; init is cheap if running. */
   if (PSX_FIO && PSX_CDC)
      mc_async_init(mc_async_file_write);
   omniplay_upstream_run();
   if (om_ps1_card_read_failed() && environ_cb)
      environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);
}

RETRO_API void omniplay_pause_guest(void)
{
   mc_async_flush_and_stop();
}

/* 1: complete checked capture barrier; 0: storage failure; -1: invalid core
 * state/policy. The worker calls only while its synchronous guest loop stops.
 * Rewrite every active card: the upstream void periodic writer can clear its
 * dirty count even when a previous file write failed. */
RETRO_API int omniplay_flush_io_checked(void)
{
   unsigned port;
   int okay = 1;
   bool used_indices[64] = { false };
   if (!PSX_FIO || !PSX_CDC || !MainRAM || strcmp(retro_save_directory, "save/PSX") || om_ps1_card_read_failed())
      return -1;
   mc_async_flush_and_stop();
   om_ps1_card_write_begin();
   for (port = 0; port < 8; ++port)
   {
      InputDevice *card = FrontIO_GetMemcardDevice(PSX_FIO, port);
      uint8_t *data;
      uint32_t size;
      int index;
      char ext[MEMCARD_EXT_MAX], path[MEMCARD_PATH_MAX];
      if (!card || !card->vt || !card->vt->GetNVSize)
         continue;
      size = card->vt->GetNVSize(card);
      if (!size)
         continue;
      if (size != (1u << 17) || !card->vt->GetNVData ||
          !card->vt->ReadNV || !card->vt->ResetNVDirtyCount)
         return -1;
      if (port == 0 && !use_mednafen_memcard0_method)
      {
         FrontIO_SaveMemcard(PSX_FIO, 0); /* captured separately as libretro SRAM */
         Memcard_PrevDC[port] = FrontIO_GetMemcardDirtyCount(PSX_FIO, port);
         Memcard_SaveDelay[port] = -1;
         continue;
      }
      index = port == 0 ? memcard_left_index : port == 1 ? memcard_right_index : (int)port;
      if (index < 0 || index > 63 || used_indices[index])
         return -1;
      used_indices[index] = true;
      snprintf(ext, sizeof(ext), "%d.mcr", index);
      MDFN_MakeFName(MDFNMKF_SAV, 0, ext, path, sizeof(path));
      if (strncmp(path, "save/PSX", 8) || (path[8] != '/' && path[8] != '\\') ||
          !path[9] || strspn(path + 9, "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-") != strlen(path + 9))
         return -1;
      data = card->vt->GetNVData(card);
      if (!data)
         return -1;
      card->vt->ReadNV(card, data, 0, size);
      if (!om_ps1_write_card(path,data,size))
      {
         okay = 0;
         continue;
      }
      card->vt->ResetNVDirtyCount(card);
      Memcard_PrevDC[port] = FrontIO_GetMemcardDirtyCount(PSX_FIO, port);
      Memcard_SaveDelay[port] = -1;
   }
   return okay;
}
