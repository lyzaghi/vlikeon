// Replaces the pinned Interface save helpers. A void board-level save must
// propagate missing/mismatched Pak writes to the explicit frontend barrier.
auto Interface::save(Memory::Writable<n16>& wram, Memory::Writable<n8>& uram,
                     Memory::Writable<n8>& lram, string name) -> bool {
  auto fp = pak ? pak->write(name) : nullptr;
  if (!fp || !fp->data()) { omni_save_failed = true; return false; }
  auto mode = fp->attribute("mode");
  size_t expected = 0;
  if (mode == "word" && wram && !uram && !lram) expected = wram.size() * 2;
  if (mode == "upper" && !wram && uram && !lram) expected = uram.size();
  if (mode == "lower" && !wram && !uram && lram) expected = lram.size();
  if (!expected || expected > 1024 * 1024 || fp->size() != expected) {
    omni_save_failed = true; return false;
  }
  if (mode == "word") for (auto address : range(wram.size())) fp->writem(wram[address], 2L);
  if (mode == "upper") for (auto address : range(uram.size())) fp->writem(uram[address], 1L);
  if (mode == "lower") for (auto address : range(lram.size())) fp->writem(lram[address], 1L);
  if (fp->offset() != expected) { omni_save_failed = true; return false; }
  return true;
}
auto Interface::save(M24C& m24c, string name) -> bool {
  auto fp = pak ? pak->write(name) : nullptr;
  if (!fp || !fp->data() || !m24c || !m24c.size() ||
      m24c.size() > 1024 * 1024 || fp->size() != m24c.size()) {
    omni_save_failed = true; return false;
  }
  fp->write(m24c.memory, m24c.size());
  if (fp->offset() != m24c.size()) { omni_save_failed = true; return false; }
  return true;
}
