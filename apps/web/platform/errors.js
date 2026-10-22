export class WebPlatformError extends Error {
  constructor(code,message){super(message);this.name='WebPlatformError';this.code=code;this.recoverable=true;}
}
export function fail(code,message){throw new WebPlatformError(code,message);}
export function requireValue(condition,code,message){if(!condition)fail(code,message);}
export function checkAbort(signal){if(signal?.aborted)fail('cancelled','The operation was cancelled.');}
export function storageError(error){
  if(error instanceof WebPlatformError)return error;
  if(error?.name==='SaveOriginError')return new WebPlatformError('corrupt_data','Saved progress origin metadata is inconsistent.');
  if(error?.name==='QuotaExceededError')return new WebPlatformError('resource_limit','Browser storage is full. Export progress and free space before trying again.');
  if(error?.name==='AbortError')return new WebPlatformError('cancelled','The storage operation was cancelled.');
  return new WebPlatformError('storage_failure','Browser storage is unavailable. Existing progress has not been replaced.');
}

export const MAX_SAVE=256*1024*1024,MAX_ROM=4*1024*1024*1024;
export const UUID=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
export const HASH=/^[0-9a-f]{64}$/;
export const identifier=value=>typeof value==='string'&&/^[A-Za-z0-9_-]{1,128}$/.test(value);
export const utf8=value=>new TextEncoder().encode(value);
export function integer(value,min,max){return Number.isSafeInteger(value)&&value>=min&&value<=max;}
