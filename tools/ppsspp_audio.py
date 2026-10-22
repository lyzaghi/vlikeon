"""Author the bounded PCM API and every selected caller at the pinned PSP parent."""
import shutil
from pinned_core_source import ROOT,file_hash


def replace(path,before,after,count=1):
    text=path.read_text(encoding='utf-8')
    if text.count(before)!=count:raise ValueError('Pinned PCM adaptation anchor changed: '+path.as_posix())
    path.write_text(text.replace(before,after),encoding='utf-8',newline='\n')


def adapt(source):
    header=source/'Core/HW/SimpleAudioDec.h';simple=source/'Core/HW/SimpleAudioDec.cpp';standalone=source/'Core/HW/Atrac3Standalone.cpp'
    replace(header,'#pragma once','#pragma once\n#include <cstddef>')
    old='int16_t *outbuf, int *outSamples';new='int16_t *outbuf, size_t outputCapacityFrames, int *outSamples'
    replace(header,old,new);replace(simple,old,new,2);replace(standalone,old,new)
    replace(header,'\t// outSamples is in stereo samples. So you have to multiply by 4 for 16-bit stereo audio to get bytes.\n\t// For Atrac3, if *outSamples != 0, it\'ll cap the number of samples to output. In this case, its value can only shrink.\n\t// TODO: Implement that in the other decoders too, if needed.',
        '\t// outputCapacityFrames is the actual destination capacity in per-channel frames.\n\t// outSamples is output-only. A null destination with zero capacity discards PCM\n\t// while still reporting decoded frames. No unsized Decode overload is exposed.')
    resampler=ROOT/'engines/native/adapters/OmniPlayAudioResampler.h'
    shutil.copy2(resampler,source/'Core/OmniPlayAudioResampler.h')
    replace(simple,'#include "Core/OmniPlayFFmpeg.h"','#include "Core/OmniPlayFFmpeg.h"\n#include "Core/OmniPlayAudioResampler.h"')
    replace(simple,'\tframe_ = av_frame_alloc();','\tframe_ = av_frame_alloc();\n\tif (!frame_) return;')
    replace(simple,'\t\treturn codec_ != 0;','\t\treturn codec_ != 0 && codecCtx_ && frame_;')
    replace(simple,'#ifdef USE_FFMPEG\n\n\tif (codecOpen_) {','#ifdef USE_FFMPEG\n\tif (!codecCtx_) return;\n\n\tif (codecOpen_) {')
    replace(simple,'\tcodecOpen_ = true;\n\treturn retval >= 0;','\tcodecOpen_ = retval >= 0;\n\treturn codecOpen_;')
    replace(simple,'void FFmpegAudioDecoder::SetChannels(int channels) {','void FFmpegAudioDecoder::SetChannels(int channels) {\n\tif (channels != 1 && channels != 2) return;')
    replace(simple,'\tSwrContext      *swrCtx_ = nullptr;','#ifdef USE_FFMPEG\n\tOmniPlayAudioResampler resampler_;\n#endif')
    replace(simple,'\tvoid SetChannels(int channels) override;','''\tvoid SetChannels(int channels) override;
\tvoid FlushBuffers() override {
#ifdef USE_FFMPEG
\t\tif (codecOpen_ && codecCtx_) avcodec_flush_buffers(codecCtx_);
\t\tresampler_.Reset();
#endif
\t}''')
    replace(simple,'\tswr_free(&swrCtx_);','\tresampler_.Reset();')
    signature='bool FFmpegAudioDecoder::Decode(const uint8_t *inbuf, int inbytes, int *inbytesConsumed, int outputChannels, int16_t *outbuf, size_t outputCapacityFrames, int *outSamples) {'
    replace(simple,signature,signature+'''
\tif (outSamples) *outSamples = 0;
\tif (inbytesConsumed) *inbytesConsumed = 0;
\tif (inbytes < 0 || inbytes > 16 * 1024 * 1024 || (inbytes && !inbuf) ||
\t    (outputChannels != 1 && outputChannels != 2) ||
\t    (outbuf && !outputCapacityFrames) || (!outbuf && outputCapacityFrames)) return false;
#ifdef USE_FFMPEG
\tif (!frame_ || !codecCtx_) return false;
#endif''')
    value=simple.read_text(encoding='utf-8');start=value.index('\t\t// Initializing the sample rate convert.');end=value.index('\n\treturn true;',start)
    value=value[:start]+'''\t\tconst int frames = resampler_.Convert(frame_, outputChannels, outbuf, outputCapacityFrames);
\t\tif (frames < 0) return false;
\t\tif (outSamples) *outSamples = frames;
\t}
'''+value[end:];simple.write_text(value,encoding='utf-8',newline='\n')
    # In no-FFmpeg builds the optional counter may still be null on discard.
    replace(simple,'\t// Zero bytes output. No need to memset.\n\t*outSamples = 0;','\t// Zero bytes output. No need to memset.\n\tif (outSamples) *outSamples = 0;')
    replace(standalone,'\n\t\tif (!codecOpen_) {','''
\t\tif (inbytes < 0 || inbytes > 16 * 1024 * 1024 || (inbytes && !inbuf) ||
\t\t    (outputChannels != 1 && outputChannels != 2) || (channels_ != 1 && channels_ != 2) ||
\t\t    (outbuf && !outputCapacityFrames) || (!outbuf && outputCapacityFrames)) return false;
\t\tif (!codecOpen_) {''')
    replace(standalone,'\t\tif (nb_samples > 0) {','''\t\tif (nb_samples < 0 || nb_samples > 2048 ||
\t\t    (outbuf && (size_t)nb_samples > outputCapacityFrames)) {
\t\t\tif (outSamples) *outSamples = 0;
\t\t\treturn false;
\t\t}
\t\tif (nb_samples > 0) {''')
    replace(standalone,'\t\t\tif (outSamples) {\n\t\t\t\tif (*outSamples != 0) {\n\t\t\t\t\tnb_samples = std::min(*outSamples, nb_samples);\n\t\t\t\t}\n\t\t\t\t*outSamples = nb_samples;\n\t\t\t}\n\t\t\treturn false;',
        '\t\t\tif (outSamples) *outSamples = 0;\n\t\t\treturn false;')
    replace(standalone,'\t\t\t// Allow capping the output samples by setting *outSamples to non-zero.\n\t\t\tif (*outSamples != 0) {\n\t\t\t\tnb_samples = std::min(*outSamples, nb_samples);\n\t\t\t}\n','')
    replace(standalone,'#include "SimpleAudioDec.h"','#include "SimpleAudioDec.h"\n#include <cmath>')
    replace(standalone,'inline int16_t clamp16(float f) {','inline int16_t clamp16(float f) {\n\tif (!std::isfinite(f)) return 0;')

    atrac=source/'Core/HLE/AtracCtx.cpp';atrac2=source/'Core/HLE/AtracCtx2.cpp';sce=source/'Core/HLE/sceAtrac.cpp'
    replace(atrac,'decoder_->Decode(BufferStart() + pos, track_.bytesPerFrame, nullptr, 2, nullptr, nullptr);','decoder_->Decode(BufferStart() + pos, track_.bytesPerFrame, nullptr, 2, nullptr, 0, nullptr);')
    replace(atrac,'decoder_->Decode(indata, track_.bytesPerFrame, &bytesConsumed, outputChannels_, (int16_t *)outbuf, &outSamples)',
        'decoder_->Decode(indata, track_.bytesPerFrame, &bytesConsumed, decodeOutputChannels, (int16_t *)outbuf, outbuf ? (size_t)track_.SamplesPerFrame() : 0, &outSamples)')
    replace(atrac,'decoder_->Decode(srcData, track_.BytesPerFrame(), bytesConsumed, channels, dstData, &outSamples);',
        'decoder_->Decode(srcData, track_.BytesPerFrame(), bytesConsumed, channels, dstData, dstData ? (size_t)track_.SamplesPerFrame() : 0, &outSamples);')
    value=atrac.read_text(encoding='utf-8');start=value.index('u32 Atrac::DecodeData(');end=value.index('\nint Atrac::SetLoopNum(',start)
    region=value[start:end].replace('outputChannels_','decodeOutputChannels')
    opening=region.index('{')+1
    region=region[:opening]+'''
\tconst int decodeOutputChannels = outbuf && outbufPtr == 0 ? 1 : outputChannels_;
\tif ((decodeOutputChannels != 1 && decodeOutputChannels != 2) || track_.SamplesPerFrame() < 1 || track_.SamplesPerFrame() > 2048) {
\t\t*SamplesNum = 0; *finish = 1; return SCE_ERROR_ATRAC_API_FAIL;
\t}
'''+region[opening:]
    atrac.write_text(value[:start]+region+value[end:],encoding='utf-8',newline='\n')
    replace(atrac,'\n\t\tif (packetAddr != 0 && MemBlockInfoDetailed()) {','''
\t\tnumSamples = std::min(std::max(maxSamples, 0), std::max(outSamples - skipped, 0));
\t\tif (outbuf && skipped && numSamples) {
\t\t\tmemmove(outbuf, outbuf + skipped * decodeOutputChannels * sizeof(s16), numSamples * decodeOutputChannels * sizeof(s16));
\t\t}
\t\tif (packetAddr != 0 && MemBlockInfoDetailed()) {''')
    replace(atrac,'\tint samplesNum;\n\tDecodeData((u8 *)dstData, 0, &samplesNum, finish, nullptr);',
        '\tint samplesNum = 0;\n\t*bytesWritten = 0;\n\tDecodeData((u8 *)dstData, 0, &samplesNum, finish, nullptr);\n\t*bytesWritten = std::clamp(samplesNum, 0, 2048);')
    replace(atrac2,'decoder_->Decode(Memory::GetPointerUnchecked(inAddr), info.sampleSize, &bytesConsumed, outputChannels_, outPtr, &outSamples)',
        'decoder_->Decode(Memory::GetPointerUnchecked(inAddr), info.sampleSize, &bytesConsumed, outputChannels_, outPtr, outPtr ? (size_t)info.SamplesPerFrame() : 0, &outSamples)')
    replace(atrac2,'decoder_->Decode(srcData, info.sampleSize, bytesConsumed, channels, dstData, &outSamples)',
        'decoder_->Decode(srcData, info.sampleSize, bytesConsumed, channels, dstData, dstData ? (size_t)info.SamplesPerFrame() : 0, &outSamples)')
    replace(atrac2,'decoder_->Decode(srcData, info.sampleSize, &bytesConsumed, 1, dstData, bytesWritten)',
        'decoder_->Decode(srcData, info.sampleSize, &bytesConsumed, 1, dstData, dstData ? 2048 : 0, bytesWritten)',2)
    replace(atrac2,'\t_dbg_assert_(samplesToDecode <= info.SamplesPerFrame());','''\tif (info.SamplesPerFrame() < 1 || info.SamplesPerFrame() > 2048 || samplesToDecode < 0 ||
\t    samplesToDecode > info.SamplesPerFrame() || (outputChannels_ != 1 && outputChannels_ != 2)) {
\t\t*SamplesNum = 0; *finish = 1; return SCE_ERROR_ATRAC_API_FAIL;
\t}''')
    # A guest-owned context may change its frame/channel shape between calls.
    # The retained temporary allocation must cover every admitted later shape.
    replace(atrac2,'decodeTemp_ = new int16_t[info.SamplesPerFrame() * outputChannels_];','decodeTemp_ = new int16_t[2048 * 2]();')
    replace(atrac2,'u32 Atrac2::DecodeInternal(u32 outbufAddr, int *SamplesNum, int *finish) {',
        'u32 Atrac2::DecodeInternal(u32 outbufAddr, int *SamplesNum, int *finish) {\n\t*SamplesNum = 0; *finish = 0;')
    replace(atrac2,'\t_dbg_assert_(bytesConsumed == info.sampleSize);','''\tif (bytesConsumed < 0 || (u32)bytesConsumed > info.sampleSize || outSamples < 0 ||
\t    outSamples > info.SamplesPerFrame() || outSamples < samplesToDecode) {
\t\tcontext_->codec.err = 0x20b; return SCE_ERROR_ATRAC_API_FAIL;
\t}''')
    replace(atrac2,'(int16_t *)Memory::GetPointerOrException(outbufAddr)',
        '(int16_t *)Memory::GetPointerWriteRangeOrException(outbufAddr, info.SamplesPerFrame() * outputChannels_ * sizeof(int16_t))')
    replace(atrac2,'u8 *outBuf = outbufAddr ? Memory::GetPointerWriteOrException(outbufAddr) : nullptr;',
        'u8 *outBuf = outbufAddr ? Memory::GetPointerWriteRangeOrException(outbufAddr, samplesToDecode * outputChannels_ * sizeof(int16_t)) : nullptr;')
    replace(atrac2,'if (!Memory::IsValidAddress(inAddr)) {','if (info.sampleSize < 1 || !Memory::IsValidRange(inAddr, info.sampleSize)) {')
    replace(atrac2,'Memory::GetPointerOrException(sas_.bufPtr[sas_.curBuffer] + sas_.streamOffset)',
        'checkedSasInput(sas_.bufPtr[sas_.curBuffer], sas_.streamOffset, info.sampleSize)')
    replace(atrac2,'void Atrac2::DecodeForSas(s16 *dstData, int *bytesWritten, int *finish) {','''void Atrac2::DecodeForSas(s16 *dstData, int *bytesWritten, int *finish) {
\t*bytesWritten = 0; *finish = 0;
\tconst auto checkedSasInput = [](u32 base, u64 offset, u32 count) -> const u8 * {
\t\tif (!count || count > 16 * 1024 * 1024 || offset > 0xFFFFFFFFu || (u64)base + offset > 0xFFFFFFFFu) return nullptr;
\t\tconst u32 address = (u32)((u64)base + offset);
\t\treturn Memory::IsValidRange(address, count) ? Memory::GetPointerUnchecked(address) : nullptr;
\t};''')
    replace(atrac2,'\tif (info.buffer) {\n\t\tsas_.curBuffer = 0;','''\tif (info.buffer) {
\t\tif (info.bufferByte < info.streamOff || info.bufferByte > 0x7FFFFFFFu) { *finish = 1; return; }
\t\tsas_.curBuffer = 0;''')
    replace(atrac2,'\tu8 assembly[1000];','''\tif ((sas_.curBuffer != 0 && sas_.curBuffer != 1) || info.sampleSize < 1 || info.sampleSize > 16 * 1024 * 1024 ||
\t    info.SamplesPerFrame() < 1 || info.SamplesPerFrame() > 2048 || sas_.streamOffset < 0 || sas_.fileOffset < 0 ||
\t    sas_.bufSize[sas_.curBuffer] > 0x7FFFFFFFu || (u64)sas_.streamOffset > sas_.bufSize[sas_.curBuffer]) { *finish = 1; return; }
\tu8 assembly[1000];''')
    replace(atrac2,'if (sas_.streamOffset + (int)info.sampleSize <= (int)sas_.bufSize[sas_.curBuffer]) {',
        'if ((u64)sas_.streamOffset + (u64)info.sampleSize <= sas_.bufSize[sas_.curBuffer]) {')
    replace(atrac2,'\t\tint bytesConsumed = 0;\n\t\tbool decodeResult = decoder_->Decode(srcData, info.sampleSize, &bytesConsumed, 1, dstData, dstData ? 2048 : 0, bytesWritten);',
        '\t\tif (!srcData) { *finish = 1; return; }\n\t\tint bytesConsumed = 0;\n\t\tbool decodeResult = decoder_->Decode(srcData, info.sampleSize, &bytesConsumed, 1, dstData, dstData ? 2048 : 0, bytesWritten);',2)
    replace(atrac2,'\t\tsas_.streamOffset += bytesConsumed;','''\t\tif (bytesConsumed < 0 || (u32)bytesConsumed > info.sampleSize) { *bytesWritten = 0; *finish = 1; return; }
\t\tsas_.streamOffset += bytesConsumed;''')
    replace(atrac2,'\t\t\tMemory::Memcpy(assembly, sas_.bufPtr[sas_.curBuffer] + sas_.streamOffset, part1Size);','''\t\t\tif (part1Size) {
\t\t\t\tconst u8 *part = checkedSasInput(sas_.bufPtr[sas_.curBuffer], sas_.streamOffset, part1Size);
\t\t\t\tif (!part) { *finish = 1; return; }
\t\t\t\tmemcpy(assembly, part, part1Size);
\t\t\t}''')
    replace(atrac2,'\t\t// Switch to the other buffer.','''\t\tif (part1Size < 0 || part2Size < 0 || (u32)part2Size > info.secondBufferByte ||
\t\t    (u64)sas_.fileOffset + info.secondBufferByte > 0x7FFFFFFFu) { *finish = 1; return; }
\t\t// Switch to the other buffer.''')
    replace(atrac2,'\t\tMemory::Memcpy(assembly + part1Size, sas_.bufPtr[sas_.curBuffer], part2Size);','''\t\tif (part2Size) {
\t\t\tconst u8 *part = checkedSasInput(sas_.bufPtr[sas_.curBuffer], 0, part2Size);
\t\t\tif (!part) { *finish = 1; return; }
\t\t\tmemcpy(assembly + part1Size, part, part2Size);
\t\t}''')
    replace(sce,'\tu8 *outPtr = outAddr ? Memory::GetPointerWriteOrException(outAddr) : nullptr;',
'''\tconst int pcmChannels = atrac->GetOutputChannels(), pcmFrames = atrac->SamplesPerFrame();
\tif ((pcmChannels != 1 && pcmChannels != 2) || pcmFrames < 1 || pcmFrames > 2048 ||
\t    (outAddr && !Memory::IsValidRange(outAddr, pcmFrames * pcmChannels * sizeof(s16))))
\t\treturn hleLogError(Log::Atrac, SCE_ERROR_ATRAC_SIZE_TOO_SMALL);
\tu8 *outPtr = outAddr ? Memory::GetPointerWriteRangeOrException(outAddr, pcmFrames * pcmChannels * sizeof(s16)) : nullptr;''')
    replace(sce,'Memory::ClampValidSizeAt(outAddr, atrac->GetOutputChannels() * 2 * atrac->SamplesPerFrame())','pcmFrames * pcmChannels * sizeof(s16)')
    replace(sce,'if (!srcp.IsValid() || !srcConsumed.IsValid() || !outp.IsValid() || !outWritten.IsValid()) {',
'''if (!srcp.IsValid() || !srcConsumed.IsValid() || !outp.IsValid() || !outWritten.IsValid() ||
\t    atrac->BytesPerFrame() < 1 || atrac->BytesPerFrame() > 16 * 1024 * 1024 ||
\t    atrac->SamplesPerFrame() < 1 || atrac->SamplesPerFrame() > 2048 ||
\t    (atrac->GetOutputChannels() != 1 && atrac->GetOutputChannels() != 2) ||
\t    !Memory::IsValidRange(sourceAddr, atrac->BytesPerFrame()) ||
\t    !Memory::IsValidRange(samplesAddr, atrac->SamplesPerFrame() * atrac->GetOutputChannels() * sizeof(s16)) ||
\t    !Memory::IsValidRange(sourceBytesConsumedAddr, 4) || !Memory::IsValidRange(sampleBytesAddr, 4)) {''')
    replace(sce,'\t\tWARN_LOG(Log::Atrac, "bad atrac ID");\n\t}',
        '\t\tWARN_LOG(Log::Atrac, "bad atrac ID");\n\t\t*SamplesNum = 0; *finish = 1; return;\n\t}',1)
    media=source/'Core/HW/MediaEngine.cpp'
    replace(media,'m_audioContext->Decode(audioFrame, frameSize, &inbytesConsumed, 2, buffer, &outSamples)',
        'm_audioContext->Decode(audioFrame, frameSize, &inbytesConsumed, 2, buffer, buffer ? 2048 : 0, &outSamples)')
    replace(simple,'decoder->Decode(&sourcebuff[nextSync], (int)sourcebuff.size() - nextSync, &inbytesConsumed, 2, (int16_t *)outbuf, &outSamples);',
        'decoder->Decode(&sourcebuff[nextSync], (int)sourcebuff.size() - nextSync, &inbytesConsumed, 2, (int16_t *)outbuf, outbuf ? PCMBufSize / 8 : 0, &outSamples);')
    replace(simple,'u32 AuCtx::AuDecode(u32 pcmAddr) {','''u32 AuCtx::AuDecode(u32 pcmAddr) {
\tif (!decoder || (nextOutputHalf != 0 && nextOutputHalf != 1) || PCMBufSize < 8 || PCMBufSize > 16 * 1024 * 1024 ||
\t    PCMBufSize % 8 || (u64)PCMBuf + PCMBufSize > 0x100000000ULL || !Memory::IsValidRange(PCMBuf, PCMBufSize) ||
\t    (pcmAddr && !Memory::IsValidRange(pcmAddr, 4)) || sourcebuff.size() > 16 * 1024 * 1024) return 0xFFFFFFFFu;''')
    replace(simple,'\t\tint inbytesConsumed = 0;\n\t\tint outSamples = 0;',
        '\t\tif (nextSync < 0 || (size_t)nextSync >= sourcebuff.size()) return 0xFFFFFFFFu;\n\t\tint inbytesConsumed = 0;\n\t\tint outSamples = 0;')
    replace(simple,'\t\toutpcmbufsize = outSamples * 2 * sizeof(int16_t);','''\t\tif (inbytesConsumed < 0 || (size_t)inbytesConsumed > sourcebuff.size() - nextSync ||
\t\t    outSamples < 0 || (u32)outSamples > PCMBufSize / 8) return 0xFFFFFFFFu;
\t\toutpcmbufsize = outSamples * 2 * sizeof(int16_t);''')
    mp3=source/'Core/HLE/sceMp3.cpp'
    replace(mp3,'const u8 *inbuff = Memory::GetPointerWriteUnchecked(sourceAddr);\n\tint16_t *outbuf = (int16_t *)Memory::GetPointerWriteUnchecked(samplesAddr);',
'''if (!Memory::IsValidRange(sourceAddr, 4096) || !Memory::IsValidRange(samplesAddr, 1152 * 2 * sizeof(s16)) ||
\t    !Memory::IsValidRange(sourceBytesConsumedAddr, 4) || !Memory::IsValidRange(sampleBytesAddr, 4)) return hleLogError(Log::ME, -1, "invalid low-level PCM span");
\tconst u8 *inbuff = Memory::GetPointerUnchecked(sourceAddr);
\tint16_t *outbuf = (int16_t *)Memory::GetPointerWriteRangeOrException(samplesAddr, 1152 * 2 * sizeof(s16));''')
    replace(mp3,'ctx->decoder->Decode(inbuff, 4096, &inbytesConsumed, 2, outbuf, &outSamples)',
        'ctx->decoder->Decode(inbuff, 4096, &inbytesConsumed, 2, outbuf, 1152, &outSamples)')
    codec=source/'Core/HLE/sceAudiocodec.cpp'
    replace(codec,'int16_t *outBuf = (int16_t *)Memory::GetPointerWriteOrException(ctx->outBuf);','int16_t *outBuf = nullptr;')
    replace(codec,'\t\tbool result = decoder->Decode(inBuf, bytesPerFrame, &inDataConsumed, outputChannels, outBuf, &outSamples);',
'''\t\tconst size_t pcmCapacity = codec == PSP_CODEC_AT3PLUS ? 2048 : codec == PSP_CODEC_AT3 ? 1024 :
\t\t    codec == PSP_CODEC_MP3 ? (ctx->fmt.mp3.version == 1 ? 1152 : 576) : ctx->fmt.aac.flag2d ? 2048 : 1024;
\t\tif (!Memory::IsValidRange(ctx->outBuf, pcmCapacity * outputChannels * sizeof(s16))) {
\t\t\tctx->err = 0x20b; return hleLogError(Log::ME, 0, "invalid PCM output span");
\t\t}
\t\toutBuf = (int16_t *)Memory::GetPointerWriteRangeOrException(ctx->outBuf, pcmCapacity * outputChannels * sizeof(s16));
\t\tbool result = decoder->Decode(inBuf, bytesPerFrame, &inDataConsumed, outputChannels, outBuf, pcmCapacity, &outSamples);''')
    text=codec.read_text(encoding='utf-8');start=text.index('static int sceAudiocodecDecode(');end=text.index('\nstatic ',start+1);region=text[start:end]
    before='if (!ctxPtr) {'
    if region.count(before)!=1:raise ValueError('The codec context admission anchor changed')
    region=region.replace(before,'if (!ctxPtr || !Memory::IsValidRange(ctxPtr, sizeof(SceAudiocodecCodec))) {',1)
    codec.write_text(text[:start]+region+text[end:],encoding='utf-8',newline='\n')
    return {'protocol':1,'output_capacity_unit':'per_channel_sample_frames','maximum_pcm_frames':8192,
        'resampler_target_channels':[1,2],'staged_pcm_commit':True,'guest_ranges_checked':True,
        'resampler_sha256':file_hash(resampler),'verification':'not_run'}
