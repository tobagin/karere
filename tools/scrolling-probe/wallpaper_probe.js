/* Reversible experiment: isolate the large decorative mask without changing it. */
(() => {
  if(window.__karereWallpaperProbe?.el?.isConnected)return true;
  const main=document.querySelector('#main');
  if(!main)throw Error('Conversation unavailable');
  const m=main.getBoundingClientRect();
  const candidates=[...document.querySelectorAll('div')].filter(e=>{
    const r=e.getBoundingClientRect(),s=getComputedStyle(e);
    return Math.abs(r.x-m.x)<3 && r.width>=m.width*.95 && r.height>=m.height*.95 &&
      s.position==='absolute' && s.maskImage!=='none';
  });
  if(candidates.length!==1)throw Error('Decorative mask is not uniquely identified');
  const el=candidates[0],style=getComputedStyle(el),r=el.getBoundingClientRect();
  window.__karereWallpaperProbe={el,
    value:el.style.getPropertyValue('will-change'),priority:el.style.getPropertyPriority('will-change'),
    set(enabled){
      if(!this.el.isConnected)throw Error('Wallpaper element replaced');
      if(enabled)this.el.style.setProperty('will-change','transform','important');
      else if(this.value)this.el.style.setProperty('will-change',this.value,this.priority);
      else this.el.style.removeProperty('will-change');
      return getComputedStyle(this.el).willChange;
    },
    geometry:{x:r.x,y:r.y,w:r.width,h:r.height,mask_is_svg:style.maskImage.includes('svg'),
      mask_repeat:style.maskRepeat,mask_size:style.maskSize,pointer_events:style.pointerEvents},
  };
  return true;
})()
