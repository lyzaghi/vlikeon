const neutral=()=>({x:0,y:0,pressed:false,inside:false});
const axis=value=>{const scaled=Math.max(-1,Math.min(1,value))*32767;return scaled<0?-Math.floor(-scaled+.5):Math.floor(scaled+.5);};
// The product canvas uses centered object-fit: contain and no CSS transform.
export function canvasPoint(canvas,geometry,clientX,clientY){
  if(!geometry||!Number.isInteger(geometry.width)||!Number.isInteger(geometry.height)||geometry.width<1||geometry.height<1||geometry.width>4096||geometry.height>4096||geometry.width*geometry.height>4096*4096||!Number.isFinite(clientX)||!Number.isFinite(clientY))return neutral();
  const box=canvas.getBoundingClientRect(),style=getComputedStyle(canvas);if(style.objectFit!=='contain'||style.transform!=='none'||box.width<=0||box.height<=0||canvas.offsetWidth<=0||canvas.offsetHeight<=0)return neutral();
  const number=value=>Number.parseFloat(value)||0,sx=box.width/canvas.offsetWidth,sy=box.height/canvas.offsetHeight;
  const left=(number(style.borderLeftWidth)+number(style.paddingLeft))*sx,right=(number(style.borderRightWidth)+number(style.paddingRight))*sx,top=(number(style.borderTopWidth)+number(style.paddingTop))*sy,bottom=(number(style.borderBottomWidth)+number(style.paddingBottom))*sy;
  const width=box.width-left-right,height=box.height-top-bottom,scale=Math.min(width/geometry.width,height/geometry.height),drawWidth=geometry.width*scale,drawHeight=geometry.height*scale;if(drawWidth<=0||drawHeight<=0)return neutral();
  const x=(clientX-box.left-left-(width-drawWidth)/2)/drawWidth,y=(clientY-box.top-top-(height-drawHeight)/2)/drawHeight,inside=x>=0&&x<1&&y>=0&&y<1;
  return{x:axis(x*2-1),y:axis(y*2-1),pressed:false,inside};
}
export class CanvasPointer{
  #canvas;#geometry;#active;#abort=new AbortController();#owner=null;#last=null;
  constructor(canvas,geometry,active){
    this.#canvas=canvas;this.#geometry=geometry;this.#active=active;canvas.dataset.pointerInput='true';
    this.#listen(canvas,'pointerdown',event=>{if(!event.isTrusted||!this.#enabled()||this.#owner||event.button!==0||!Number.isInteger(event.pointerId)||event.pointerId<0)return;const position=canvasPoint(canvas,this.#geometry(),event.clientX,event.clientY);if(!position.inside)return;event.preventDefault();try{canvas.setPointerCapture(event.pointerId);}catch{return;}canvas.focus({preventScroll:true});this.#owner={id:event.pointerId,type:event.pointerType};this.#last={x:event.clientX,y:event.clientY};});
    this.#listen(canvas,'pointermove',event=>{if(!event.isTrusted||!this.#enabled())return;if(this.#owner&&event.pointerId!==this.#owner.id)return;if(this.#owner&&!(event.buttons&1)){this.clear();return;}if(!this.#owner&&event.pointerType==='touch')return;this.#last={x:event.clientX,y:event.clientY};if(this.#owner)event.preventDefault();});
    this.#listen(canvas,'pointerup',event=>{if(this.#owner?.id!==event.pointerId)return;const type=this.#owner.type;this.#owner=null;this.#last=type==='touch'?null:{x:event.clientX,y:event.clientY};});
    this.#listen(canvas,'pointercancel',event=>{if(this.#owner?.id===event.pointerId)this.clear();});this.#listen(canvas,'lostpointercapture',event=>{if(this.#owner?.id===event.pointerId)this.clear();});this.#listen(canvas,'pointerleave',()=>{if(!this.#owner)this.#last=null;});
    this.#listen(canvas,'contextmenu',event=>{if(this.#enabled())event.preventDefault();});
    this.#listen(window,'blur',()=>this.clear());this.#listen(window,'pagehide',()=>this.clear());this.#listen(window,'resize',()=>this.clear());this.#listen(window,'scroll',()=>this.clear(),{capture:true,passive:true});this.#listen(document,'visibilitychange',()=>{if(document.hidden)this.clear();});
    if(window.visualViewport){this.#listen(window.visualViewport,'resize',()=>this.clear());this.#listen(window.visualViewport,'scroll',()=>this.clear());}
  }
  #listen(target,type,handler,options={}){target.addEventListener(type,handler,{...options,signal:this.#abort.signal});}
  #enabled(){return Boolean(this.#active()&&this.#canvas.isConnected&&!document.hidden&&document.hasFocus());}
  snapshot(){if(!this.#enabled()||!this.#last){this.clear();return neutral();}const point=canvasPoint(this.#canvas,this.#geometry(),this.#last.x,this.#last.y);point.pressed=Boolean(this.#owner&&point.inside);return point;}
  clear(){const owner=this.#owner;this.#owner=null;this.#last=null;if(owner)try{if(this.#canvas.hasPointerCapture(owner.id))this.#canvas.releasePointerCapture(owner.id);}catch{}}
  dispose(){this.#abort.abort();this.clear();delete this.#canvas.dataset.pointerInput;}
}
