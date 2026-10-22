
/* Appended to the pinned libretro.c, where its card indices/private state live.
 * The worker invokes this only between retro_run calls in an unthreaded core.
 * Read every card, even if an upstream periodic write reset its dirty counter:
 * the upstream void writer does not reliably propagate short-write failures. */
#if (defined(HAVE_THREADS) && HAVE_THREADS) || defined(__EMSCRIPTEN_PTHREADS__) || defined(HAVE_LIGHTREC)
#error OmniPlay saved files require the software unthreaded interpreter profile
#endif
int om_web_flush_save_io(void)
{
   unsigned port;
   int okay = 1;
   bool used_indices[64] = { false };
   if (!PSX_FIO || !MainRAM || !PSX_CDC)
      return 0;
   for (port = 0; port < 8; ++port)
   {
      InputDevice *card = FrontIO_GetMemcardDevice(PSX_FIO, port);
      uint8_t *data;
      uint32_t size;
      int index, length, flushed, closed;
      char ext[MEMCARD_EXT_MAX], path[MEMCARD_PATH_MAX];
      char temporary[MEMCARD_PATH_MAX + 16];
      RFILE *stream;
      int64_t written;
      if (!card || !card->vt || !card->vt->GetNVSize)
         continue;
      size = card->vt->GetNVSize(card);
      if (!size)
         continue;
      if (size != (1u << 17) || !card->vt->GetNVData ||
          !card->vt->ReadNV || !card->vt->ResetNVDirtyCount)
      {
         okay = 0;
         continue;
      }
      if (port == 0 && !use_mednafen_memcard0_method)
      {
         /* The first card remains the ordinary libretro SRAM region. */
         FrontIO_SaveMemcard(PSX_FIO, 0);
         Memcard_PrevDC[port] = FrontIO_GetMemcardDirtyCount(PSX_FIO, port);
         Memcard_SaveDelay[port] = -1;
         continue;
      }
      index = port == 0 ? memcard_left_index : port == 1 ? memcard_right_index : (int)port;
      if (index < 0 || index > 63) { okay = 0; continue; }
      if (used_indices[index]) { okay = 0; continue; }
      used_indices[index] = true;
      snprintf(ext, sizeof(ext), "%d.mcr", index);
      MDFN_MakeFName(MDFNMKF_SAV, 0, ext, path, sizeof(path));
      if (strncmp(path, "/saves/", 7) || !path[7]) { okay = 0; continue; }
      length = snprintf(temporary, sizeof(temporary), "%s.omtmp", path);
      if (length < 0 || (size_t)length >= sizeof(temporary)) { okay = 0; continue; }
      data = card->vt->GetNVData(card);
      if (!data) { okay = 0; continue; }
      card->vt->ReadNV(card, data, 0, size);
      stream = filestream_open(temporary, RETRO_VFS_FILE_ACCESS_WRITE,
            RETRO_VFS_FILE_ACCESS_HINT_NONE);
      if (!stream) { okay = 0; continue; }
      written = filestream_write(stream, data, size);
      flushed = filestream_flush(stream);
      closed = filestream_close(stream);
      if (written != (int64_t)size || flushed != 0 || closed != 0 ||
          filestream_rename(temporary, path) != 0)
      {
         filestream_delete(temporary);
         okay = 0;
         continue;
      }
      card->vt->ResetNVDirtyCount(card);
      Memcard_PrevDC[port] = FrontIO_GetMemcardDirtyCount(PSX_FIO, port);
      Memcard_SaveDelay[port] = -1;
   }
   return okay;
}
