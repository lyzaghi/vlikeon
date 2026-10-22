// SHA-256 as specified by FIPS 180-4. The streaming interface bounds memory for
// user files and makes no claim to replace WebCrypto for keys or authentication.
const K=new Uint32Array([
  0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
  0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
  0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
  0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
  0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
  0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
  0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
  0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2]);
const rotate=(value,count)=>(value>>>count)|(value<<(32-count));
export class Sha256 {
  #state=new Uint32Array([0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19]);
  #buffer=new Uint8Array(64);#words=new Uint32Array(64);#used=0;#length=0;#finished=false;
  update(bytes){
    if(this.#finished||!(bytes instanceof Uint8Array)||!Number.isSafeInteger(this.#length+bytes.byteLength))throw new Error('Invalid hash input.');
    this.#length+=bytes.byteLength;let at=0;
    if(this.#used){const n=Math.min(64-this.#used,bytes.length);this.#buffer.set(bytes.subarray(0,n),this.#used);this.#used+=n;at=n;if(this.#used===64){this.#block(this.#buffer,0);this.#used=0;}}
    while(at+64<=bytes.length){this.#block(bytes,at);at+=64;}
    if(at<bytes.length){this.#buffer.set(bytes.subarray(at),0);this.#used=bytes.length-at;}return this;
  }
  clone(){if(this.#finished)throw new Error('Hash already finalized.');const value=new Sha256();value.#state.set(this.#state);value.#buffer.set(this.#buffer);value.#used=this.#used;value.#length=this.#length;return value;}
  #block(bytes,offset){
    const w=this.#words;
    for(let i=0;i<16;i++){const at=offset+i*4;w[i]=((bytes[at]<<24)|(bytes[at+1]<<16)|(bytes[at+2]<<8)|bytes[at+3])>>>0;}
    for(let i=16;i<64;i++){const a=w[i-15],b=w[i-2];w[i]=(w[i-16]+(rotate(a,7)^rotate(a,18)^(a>>>3))+w[i-7]+(rotate(b,17)^rotate(b,19)^(b>>>10)))>>>0;}
    let[a,b,c,d,e,f,g,h]=this.#state;
    for(let i=0;i<64;i++){
      const one=(h+(rotate(e,6)^rotate(e,11)^rotate(e,25))+((e&f)^(~e&g))+K[i]+w[i])>>>0;
      const two=((rotate(a,2)^rotate(a,13)^rotate(a,22))+((a&b)^(a&c)^(b&c)))>>>0;
      h=g;g=f;f=e;e=(d+one)>>>0;d=c;c=b;b=a;a=(one+two)>>>0;
    }
    const value=[a,b,c,d,e,f,g,h];for(let i=0;i<8;i++)this.#state[i]=(this.#state[i]+value[i])>>>0;
  }
  hex(){
    if(this.#finished)throw new Error('Hash already finalized.');this.#finished=true;
    this.#buffer[this.#used++]=0x80;
    if(this.#used>56){this.#buffer.fill(0,this.#used);this.#block(this.#buffer,0);this.#used=0;}
    this.#buffer.fill(0,this.#used,56);const view=new DataView(this.#buffer.buffer);
    view.setUint32(56,Math.floor(this.#length/0x20000000),false);view.setUint32(60,(this.#length*8)>>>0,false);this.#block(this.#buffer,0);
    return Array.from(this.#state,value=>value.toString(16).padStart(8,'0')).join('');
  }
}

export async function hashBlob(blob,{signal,onProgress=()=>{}}={}){
  const hash=new Sha256();let offset=0;
  while(offset<blob.size){
    if(signal?.aborted)throw new DOMException('Hash cancelled','AbortError');
    const end=Math.min(blob.size,offset+256*1024);hash.update(new Uint8Array(await blob.slice(offset,end).arrayBuffer()));offset=end;onProgress(offset);
  }
  if(signal?.aborted)throw new DOMException('Hash cancelled','AbortError');return hash.hex();
}
