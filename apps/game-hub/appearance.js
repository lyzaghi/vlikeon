import {currentLocale} from './i18n.js';
export const DEFAULT_APPEARANCE=Object.freeze({accent:'lime',library_layout:'cards',library_density:'comfortable'});
const CHOICES=Object.freeze({accent:['lime','blue','violet','amber'],library_layout:['cards','list'],library_density:['comfortable','compact']});
const LABELS={vi:{title:'Không gian chơi của bạn',description:'Chọn màu và cách sắp xếp thư viện. Các tùy chỉnh này lưu trong hồ sơ hiện tại.',accent:'Màu nhấn',library_layout:'Cách hiển thị game',library_density:'Khoảng cách thẻ',lime:'Xanh lá',blue:'Xanh biển',violet:'Tím',amber:'Hổ phách',cards:'Thẻ bìa',list:'Danh sách',comfortable:'Thoáng',compact:'Gọn',reset:'Khôi phục giao diện mặc định'},en:{title:'Make your play space yours',description:'Choose a color and library layout. These settings stay in this profile.',accent:'Accent color',library_layout:'Library layout',library_density:'Card spacing',lime:'Lime',blue:'Blue',violet:'Violet',amber:'Amber',cards:'Cover cards',list:'List',comfortable:'Comfortable',compact:'Compact',reset:'Restore default appearance'}};
const label=key=>LABELS[currentLocale()]?.[key]??LABELS.en[key];
function element(tag,className,text){const node=document.createElement(tag);if(className)node.className=className;if(text!==undefined)node.textContent=text;return node;}
export function normalizedAppearance(value){const result={...DEFAULT_APPEARANCE};if(value&&typeof value==='object')for(const[key,choices]of Object.entries(CHOICES))if(choices.includes(value[key]))result[key]=value[key];return result;}
export function applyAppearance(value){const next=normalizedAppearance(value);document.documentElement.dataset.accent=next.accent;document.documentElement.dataset.libraryLayout=next.library_layout;document.documentElement.dataset.libraryDensity=next.library_density;return next;}
export class AppearanceControls{
  #form;#nodes=new Map();#section;#saved={...DEFAULT_APPEARANCE};
  constructor(form){
    this.#form=form;const section=element('fieldset','appearance-controls'),legend=element('legend');section.append(legend,element('p','muted small'));this.#section=section;
    for(const[key,choices]of Object.entries(CHOICES)){
      const field=element('label'),name=element('span'),select=element('select');select.name='appearance_'+key;
      for(const choice of choices){const option=element('option');option.value=choice;select.append(option);}field.append(name,select);section.append(field);this.#nodes.set(key,{name,select});select.addEventListener('change',()=>applyAppearance(this.value()));
    }
    const reset=element('button','button button-secondary');reset.type='button';reset.addEventListener('click',()=>this.preview(DEFAULT_APPEARANCE));section.append(reset);const anchor=form.querySelector('button[type=submit]');form.insertBefore(section,anchor);document.addEventListener('omniplay_locale_changed',()=>this.#translate());this.#translate();
  }
  #translate(){this.#section.querySelector('legend').textContent=label('title');this.#section.querySelector('p').textContent=label('description');for(const[key,{name,select}]of this.#nodes){name.textContent=label(key);for(const option of select.options)option.textContent=label(option.value);}this.#section.querySelector('button').textContent=label('reset');}
  value(){return normalizedAppearance(Object.fromEntries([...this.#nodes].map(([key,{select}])=>[key,select.value])));}
  preview(value){const appearance=applyAppearance(value);for(const[key,{select}]of this.#nodes)select.value=appearance[key];}
  setSaved(value){this.#saved=normalizedAppearance(value);this.preview(this.#saved);}
  revertPreview(){this.preview(this.#saved);}
}
