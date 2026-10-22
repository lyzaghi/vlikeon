// Authored adapter for the pinned software-renderer pthread profile.
// No guest frame runs concurrently with a frontend save/restore command.
extern "C" RETRO_API int om_web_flush_save_io(void) {
   if (!Memory::IsActive() || !gpu || Libretro::useEmuThread)
      return 0;
   // Drawing is drained even for a stalled display list. Savedata dialog I/O
   // is a separate producer from the sceIo manager, so both must be joined.
   gpu->FlushPendingDrawing();
   __UtilityWaitForIO();
   __IoWaitForAsync();
   return std::fflush(nullptr) == 0 ? 1 : 0;
}
