import { invoke } from "@tauri-apps/api/core";
import { createLiquidLoop } from "./liquid-loop";

// Capture -> separate optical background follows dynamic-island's approach:
// https://github.com/molian313/dynamic-island. Independent renderer; no source copied.
const NS = "http://www.w3.org/2000/svg";
const BEZEL = 28;
const DISPLACEMENT = 28;
const IDLE_FRAME_MS = 1000 / 30;
const DRAG_FRAME_MS = 1000 / 60;
const REDUCED_FRAME_MS = 250;
interface SurfaceLayer {
  surface: HTMLElement; layer: HTMLDivElement; canvas: HTMLCanvasElement;
  filter: SVGFilterElement; key: string;
}
interface CapturedFrame { entry: SurfaceLayer; bitmap: ImageBitmap; key: string }

export function mountLiquidGlass(shell: HTMLElement, rail: HTMLElement, detail: HTMLElement) {
  const defs = document.createElementNS(NS, "svg");
  defs.classList.add("liquid-filter-defs");
  defs.setAttribute("aria-hidden", "true");
  defs.innerHTML = "<defs></defs>";
  shell.append(defs);
  const motion = matchMedia("(prefers-reduced-motion: reduce)");
  const entries: SurfaceLayer[] = [rail, detail].map((surface, index) => {
    const layer = document.createElement("div");
    layer.className = "liquid-layer"; layer.setAttribute("aria-hidden", "true"); layer.hidden = true;
    const canvas = document.createElement("canvas"); layer.append(canvas);
    if (index === 0) surface.querySelector(".notch-finish")!.before(layer);
    else surface.prepend(layer);
    const filter = document.createElementNS(NS, "filter");
    filter.id = `liquid-lens-${index}`;
    filter.setAttribute("color-interpolation-filters", "sRGB");
    filter.setAttribute("x", "0"); filter.setAttribute("y", "0");
    filter.setAttribute("width", "100%"); filter.setAttribute("height", "100%");
    filter.innerHTML = `<feImage result="map" preserveAspectRatio="none"/>
      <feDisplacementMap in="SourceGraphic" in2="map" scale="${DISPLACEMENT * .93}" xChannelSelector="R" yChannelSelector="G" result="r"/>
      <feColorMatrix in="r" type="matrix" values="1 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 1 0" result="red"/>
      <feDisplacementMap in="SourceGraphic" in2="map" scale="${DISPLACEMENT}" xChannelSelector="R" yChannelSelector="G" result="g"/>
      <feColorMatrix in="g" type="matrix" values="0 0 0 0 0  0 1 0 0 0  0 0 0 0 0  0 0 0 1 0" result="green"/>
      <feDisplacementMap in="SourceGraphic" in2="map" scale="${DISPLACEMENT * 1.07}" xChannelSelector="R" yChannelSelector="G" result="b"/>
      <feColorMatrix in="b" type="matrix" values="0 0 0 0 0  0 0 0 0 0  0 0 1 0 0  0 0 0 1 0" result="blue"/>
      <feBlend in="red" in2="green" mode="screen" result="rg"/><feBlend in="rg" in2="blue" mode="screen"/>`;
    defs.firstElementChild!.append(filter);
    canvas.style.filter = `url(#${filter.id}) blur(${index === 0 ? .55 : 8}px) saturate(1.1) brightness(1.04)`;
    return {surface,layer,canvas,filter,key:""};
  });
  let enabled = false, destroyed = false;
  const clear = () => entries.forEach(entry => { entry.layer.hidden = true; });
  const loop = createLiquidLoop<CapturedFrame[]>({
    request: cb => requestAnimationFrame(cb), cancel: id => cancelAnimationFrame(id),
    interval: () => motion.matches ? REDUCED_FRAME_MS
      : rail.classList.contains("is-dragging") ? DRAG_FRAME_MS : IDLE_FRAME_MS,
    capture, paint: frames => {
      let painted = false;
      frames.forEach(({entry,bitmap,key}) => {
        if (entry.key !== key || entry.surface.hidden) return;
        entry.canvas.getContext("2d")!.drawImage(bitmap,0,0,entry.canvas.width,entry.canvas.height);
        entry.layer.hidden = false;
        painted = true;
      });
      if (painted) shell.dataset.liquidStatus = "ready";
    },
    dispose: frames => frames.forEach(({bitmap}) => bitmap.close()),
    release: async () => { await invoke("release_liquid_backdrop"); },
    fail: error => {
      clear();
      shell.dataset.liquidStatus = "unavailable";
      console.error("Liquid backdrop:",error);
    },
  });
  function sync() {
    const visible = enabled && !document.hidden && (!rail.classList.contains("is-folded") || !detail.hidden);
    if (!visible) clear();
    loop.setEnabled(visible);
  }
  const events = new AbortController();
  document.addEventListener("visibilitychange",sync,{signal:events.signal});
  const observer = new MutationObserver(sync);
  observer.observe(rail,{attributes:true,attributeFilter:["class"]});
  observer.observe(detail,{attributes:true,attributeFilter:["hidden"]});

  function mapFor(entry: SurfaceLayer, width: number, height: number) {
    const contentScale = Number(getComputedStyle(entry.surface).getPropertyValue("--notch-scale")) || 1;
    const bezel = BEZEL * contentScale;
    entry.filter.querySelectorAll("feDisplacementMap").forEach((node, index) => {
      const dispersion = [.93, 1, 1.07][index];
      node.setAttribute("scale", String(DISPLACEMENT * contentScale * dispersion));
    });
    const mask = document.createElement('canvas'); mask.width=width; mask.height=height;
    const ctx = mask.getContext('2d', {willReadFrequently:true})!;
    ctx.fillStyle='#fff';
    if (entry.surface === rail) {
      const path = rail.querySelector<SVGPathElement>('#notch-outline')!;
      const m = path.transform.baseVal.consolidate()?.matrix;
      if (m) ctx.setTransform(m.a,m.b,m.c,m.d,m.e,m.f);
      ctx.fill(new Path2D(path.getAttribute('d')!)); ctx.resetTransform();
    } else {ctx.beginPath(); ctx.roundRect(0,0,width,height,16 * contentScale); ctx.fill();}
    const pixels=ctx.getImageData(0,0,width,height).data;
    const dist=new Float32Array(width*height);
    for(let y=0;y<height;y++)for(let x=0;x<width;x++) {
      const i=y*width+x;
      dist[i]=pixels[i*4+3]<128 ? 0 : Math.min(x+1,y+1,width-x,height-y,bezel+2);
    }
    for(let y=1;y<height;y++)for(let x=1;x<width;x++) {
      const i=y*width+x;dist[i]=Math.min(dist[i],dist[i-1]+1,dist[i-width]+1,dist[i-width-1]+Math.SQRT2);
    }
    for(let y=height-2;y>=0;y--)for(let x=width-2;x>=0;x--) {
      const i=y*width+x;dist[i]=Math.min(dist[i],dist[i+1]+1,dist[i+width]+1,dist[i+width+1]+Math.SQRT2);
    }
    const map=ctx.createImageData(width,height);
    for(let y=0;y<height;y++)for(let x=0;x<width;x++) {
      const i=y*width+x, d=dist[i];
      const gx=(dist[y*width+Math.min(width-1,x+1)]-dist[y*width+Math.max(0,x-1)])/2;
      const gy=(dist[Math.min(height-1,y+1)*width+x]-dist[Math.max(0,y-1)*width+x])/2;
      const length=Math.hypot(gx,gy)||1;
      const bend=d>0&&d<bezel ? (1-d/bezel)**2 : 0;
      map.data[i*4]=128+gx/length*bend*116;
      map.data[i*4+1]=128+gy/length*bend*116;
      map.data[i*4+2]=128;map.data[i*4+3]=255;
    }
    ctx.putImageData(map,0,0);
    entry.filter.querySelector('feImage')!.setAttribute('href',mask.toDataURL());
    entry.canvas.width=width;entry.canvas.height=height;
  }

  async function capture(): Promise<CapturedFrame[]> {
    const frames: CapturedFrame[] = [];
    try {
      for (const entry of entries) {
        if (entry.surface.hidden || (entry.surface === rail && rail.classList.contains("is-folded"))) continue;
        const rect = entry.surface.getBoundingClientRect();
        const width = Math.round(rect.width), height = Math.round(rect.height);
        if (!width || !height) continue;
        const key = `${width}:${height}:${rail.dataset.edge}`;
        if (key !== entry.key) { mapFor(entry,width,height); entry.key=key; }
        const ratio = devicePixelRatio;
        const x=Math.max(0,Math.round(rect.x*ratio)), y=Math.max(0,Math.round(rect.y*ratio));
        const w=Math.min(Math.round(width*ratio),Math.round(innerWidth*ratio)-x);
        const h=Math.min(Math.round(height*ratio),Math.round(innerHeight*ratio)-y);
        // During a native canvas transition wait for a valid local rectangle.
        if (w<=0 || h<=0) continue;
        const data = await invoke<ArrayBuffer>("capture_liquid_backdrop",{x,y,width:w,height:h});
        if (!data.byteLength) continue; // Native geometry changed before the DOM caught up.
        const bitmap = await createImageBitmap(new Blob([data],{type:"image/bmp"}));
        frames.push({entry,bitmap,key});
      }
      return frames;
    } catch(error) { frames.forEach(({bitmap})=>bitmap.close()); throw error; }
  }
  return {
    setEnabled(value: boolean) {
      if (destroyed || enabled === value) return;
      enabled=value;
      delete shell.dataset.liquidStatus;
      sync();
    },
    destroy() {
      destroyed=true; loop.destroy(); observer.disconnect(); events.abort();
      entries.forEach(entry=>entry.layer.remove()); defs.remove();
    },
  };
}
