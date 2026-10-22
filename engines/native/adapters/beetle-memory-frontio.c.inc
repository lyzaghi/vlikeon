/* Included inside the pinned FrontIO implementation. Source recipe replaces
 * only its two file-managed-card functions; SRAM/device behavior stays local. */
void FrontIO_LoadMemcardFromPath(FrontIO *self_, unsigned int which, const char *path, bool force_load)
{
   InputDevice* card;
   uint8_t* staged;
   int result;
   assert(which<8);
   card=self_->DevicesMC[which];
   if (!card || !card->vt || !card->vt->GetNVSize || !card->vt->GetNVSize(card)) return;
   if (card->vt->GetNVSize(card)!=(1u<<17) || !card->vt->WriteNV || !card->vt->ResetNVDirtyCount)
   {om_ps1_read_card(NULL,NULL,0);return;}
   staged=(uint8_t*)malloc(1u<<17);
   if (!staged) {om_ps1_read_card(NULL,NULL,0);return;}
   result=om_ps1_read_card(path,staged,1u<<17);
   if (result==1)
   {
      Device_Memcard_Power(card);
      card->vt->WriteNV(card,staged,0,1u<<17);
      card->vt->ResetNVDirtyCount(card);
   }
   else if (result==0 && force_load)
   {
      Device_Memcard_Power(card);
      Device_Memcard_Format(card);
   }
   free(staged);
}

void FrontIO_SaveMemcardToPath(FrontIO *self_, unsigned int which, const char *path, bool force_save)
{
   InputDevice* card;
   uint8_t* data;
   assert(which<8);
   card=self_->DevicesMC[which];
   if (!card || !card->vt || !card->vt->GetNVSize || !card->vt->GetNVSize(card)) return;
   if (card->vt->GetNVSize(card)!=(1u<<17) || !card->vt->GetNVDirtyCount ||
       !card->vt->GetNVData || !card->vt->ReadNV || !card->vt->ResetNVDirtyCount)
   {om_ps1_write_card(NULL,NULL,0);return;}
   if (!force_save && !card->vt->GetNVDirtyCount(card)) return;
   data=card->vt->GetNVData(card);
   if (!data) {om_ps1_write_card(NULL,NULL,0);return;}
   card->vt->ReadNV(card,data,0,1u<<17);
   if (om_ps1_write_card(path,data,1u<<17)) card->vt->ResetNVDirtyCount(card);
}
