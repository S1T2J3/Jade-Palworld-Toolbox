// Both atlas pages use normalized image coordinates: the top-left corner is (0,0).
function createMapCamera({viewport,sheet,markerLayer,zoomLabel,zoomIn,zoomOut,fitButton,onDraw,onBlank,onPoint,onMove,uiSelector,markerSelector}) {
  const maxZoom=16, pointers=new Map();
  let scale=1, centerX=.5, centerY=.5, frame=0, settle=0, dragEnded=false;
  let width=0,height=0,baseWidth=0,baseHeight=0,anchor=null,previousZoom=0,dragRect;
  const scene=sheet.parentElement;
  let contentChanged=true,pixelRatio=1;
  function measure() {
    width=viewport.clientWidth;height=viewport.clientHeight;
    pixelRatio=window.devicePixelRatio||1;
    const aspect=sheet.naturalWidth && sheet.naturalHeight ? sheet.naturalWidth/sheet.naturalHeight : 1;
    baseHeight=Math.min(height,width/aspect);baseWidth=baseHeight*aspect;
  }
  function geometry() {
    if (!width || !height) return null;
    const w=baseWidth*scale,h=baseHeight*scale;
    const keep=(value,screen,size)=>{
      const half=Math.min(.5,screen/(size*2));
      return Math.min(1-half,Math.max(half,value));
    };
    centerX=keep(centerX,width,w);centerY=keep(centerY,height,h);
    // Align the moving texture to physical pixels to avoid alternating
    // interpolation offsets on small movements and fractional Windows scaling.
    const x=Math.round((width/2-centerX*w)*pixelRatio)/pixelRatio;
    const y=Math.round((height/2-centerY*h)*pixelRatio)/pixelRatio;
    return {w,h,x,y,width,height};
  }
  function positionScene(g,ratio) {
    // Both the terrain and the pins inherit this one camera transform.
    scene.style.transform=`translate3d(${g.x}px,${g.y}px,0) scale(${ratio})`;
    if(previousZoom===scale) return;
    previousZoom=scale;
    const text=Number(scale.toFixed(1))+'×';
    if(zoomLabel.textContent!==text) zoomLabel.textContent=text;
    zoomIn.disabled=scale>=maxZoom-.001;zoomOut.disabled=scale<=1.001;
  }
  function draw() {
    frame=0;
    if(contentChanged || !anchor)measure();
    const g=geometry();if (!g) return;
    g.panOnly=!contentChanged && anchor?.w===g.w && anchor?.h===g.h;
    if(!g.panOnly) {
      sheet.style.width=baseWidth+'px';sheet.style.height=baseHeight+'px';
      sheet.style.transform='scale('+scale+')';
    }
    contentChanged=false;
    g.padX=Math.min(280,Math.ceil(width*.34));g.padY=Math.min(200,Math.ceil(height*.34));
    anchor=g;
    positionScene(g,1);
    onDraw(g);
  }
  function scheduleDraw() {if(!frame) frame=requestAnimationFrame(draw);}
  function refresh() {contentChanged=true;scheduleDraw();}
  function move(zooming) {
    const g=geometry();if(!g) return;
    onMove?.();
    if(!anchor){refresh();return;}
    const ratio=g.w/anchor.w;
    positionScene(g,ratio);
    const dx=g.x-anchor.x*ratio,dy=g.y-anchor.y*ratio;
    if(ratio<.77||ratio>1.3 || (!anchor.panComplete && (dx-anchor.padX*ratio>0 || dy-anchor.padY*ratio>0 ||
       dx+(anchor.width+anchor.padX)*ratio<width || dy+(anchor.height+anchor.padY)*ratio<height))) scheduleDraw();
    // Wheel/trackpad events can span many frames. Restore exact icon sizes
    // and cluster distances once the gesture ends, including fractional zoom.
    if(zooming){
      clearTimeout(settle);
      settle=setTimeout(()=>{settle=0;if(anchor && geometry()?.w!==anchor.w)refresh();},95);
    }
  }
  function fit() {scale=1;centerX=.5;centerY=.5;refresh();}
  function center(u,v,minZoom=scale) {
    scale=Math.min(maxZoom,Math.max(scale,minZoom,1));centerX=u;centerY=v;refresh();
  }
  function zoomAt(next,x,y,midDX=0,midDY=0) {
    const g=geometry();if (!g) return;
    const u=centerX+(x-g.width/2)/g.w,v=centerY+(y-g.height/2)/g.h;
    next=Math.max(1,Math.min(maxZoom,next));
    const factor=next/scale;scale=next;
    centerX=u-(x+midDX-g.width/2)/(g.w*factor);centerY=v-(y+midDY-g.height/2)/(g.h*factor);
    move(true);
  }
  function pan(dx,dy) {
    const g=geometry();if (!g) return;
    centerX-=dx/g.w;centerY-=dy/g.h;move(false);
  }
  const relative=(e,rect)=>({x:e.clientX-rect.left,y:e.clientY-rect.top});
  const distance=(a,b)=>Math.hypot(a.x-b.x,a.y-b.y);
  const zoomMiddle=factor=>zoomAt(scale*factor,width/2,height/2);
  function bindControl(button,action) {
    let touch=null,handled=false;
    button.addEventListener('pointerdown',e=>{
      handled=false;
      touch=e.pointerType==='touch'&&e.isPrimary?{id:e.pointerId,x:e.clientX,y:e.clientY}:null;
    });
    button.addEventListener('pointercancel',()=>{touch=null;});
    button.addEventListener('pointerup',e=>{
      // A browser may suppress the compatibility click just after a pinch.
      if(touch?.id===e.pointerId && Math.hypot(e.clientX-touch.x,e.clientY-touch.y)<8){
        handled=true;action();
      }
      touch=null;
    });
    button.addEventListener('click',e=>{
      if(!handled||!e.detail)action();
      handled=false;
    });
  }
  bindControl(zoomIn,()=>zoomMiddle(1.6));
  bindControl(zoomOut,()=>zoomMiddle(1/1.6));
  bindControl(fitButton,fit);
  sheet.addEventListener('load',refresh);
  viewport.addEventListener('wheel',e=>{
    if (e.target.closest(uiSelector)) return;
    e.preventDefault();
    const pos=relative(e,viewport.getBoundingClientRect());
    const delta=e.deltaY*(e.deltaMode===1?16:e.deltaMode===2?height:1);
    zoomAt(scale*Math.exp(-Math.max(-500,Math.min(500,delta))*.0025),pos.x,pos.y);
  },{passive:false});
  viewport.addEventListener('pointerdown',e=>{
    if(e.button!==0 || e.target.closest(uiSelector)) return;
    if(!pointers.size) dragRect=viewport.getBoundingClientRect();
    dragEnded=false;
    const pos=relative(e,dragRect);
    pointers.set(e.pointerId,{...pos,start:pos,drag:false});
    if(!e.target.closest(markerSelector)){viewport.setPointerCapture(e.pointerId);viewport.focus({preventScroll:true});}
    if(pointers.size>1) for(const [id,p] of pointers){p.drag=true;viewport.setPointerCapture(id);}
  });
  viewport.addEventListener('pointermove',e=>{
    const p=pointers.get(e.pointerId);if(!p) return;
    const now=relative(e,dragRect);
    if(pointers.size>1) {
      const other=[...pointers.entries()].find(([id])=>id!==e.pointerId)[1];
      const old=distance(p,other), next=distance(now,other);
      if(old>2 && next>2) zoomAt(scale*next/old,(p.x+other.x)/2,(p.y+other.y)/2,(now.x-p.x)/2,(now.y-p.y)/2);
      Object.assign(p,now);return;
    }
    if(!p.drag && distance(p.start,now)<(e.pointerType==='mouse'?1.5:4)) return;
    if(!p.drag){p.drag=true;viewport.setPointerCapture(e.pointerId);viewport.classList.add('wm-panning');}
    pan(now.x-p.x,now.y-p.y);Object.assign(p,now);
  });
  function endPointer(e) {
    const p=pointers.get(e.pointerId);if(!p)return;
    if(p.drag) dragEnded=true;
    pointers.delete(e.pointerId);
    if(!pointers.size) viewport.classList.remove('wm-panning');
  }
  for(const event of ['pointerup','pointercancel']) viewport.addEventListener(event,endPointer);
  viewport.addEventListener('lostpointercapture',e=>{if(e.target===viewport&&!viewport.hasPointerCapture(e.pointerId))endPointer(e);});
  viewport.addEventListener('click',e=>{
    if(e.composedPath().some(node=>node.matches?.(uiSelector))||(e.detail && dragEnded))return;
    const marker=e.target.closest(markerSelector);
    if(marker) onPoint(marker);else onBlank(e);
  });
  viewport.addEventListener('keydown',e=>{
    if(e.target!==viewport) return;
    const step=e.shiftKey?100:45;
    if(e.key==='Tab'&&!e.shiftKey){
      const pins=visibleMarkers();
      if(!pins.some(pin=>pin.tabIndex===0)){
        const old=markerLayer.querySelector('[tabindex="0"]');
        if(old)old.tabIndex=-1;
        if(pins.length)pins[0].tabIndex=0;
      }
      return;
    }
    if(e.key==='Home') fit();
    else if(e.key==='Escape') onBlank(e);
    else if(e.key==='+'||e.key==='=') zoomMiddle(1.6);
    else if(e.key==='-') zoomMiddle(1/1.6);
    else if(e.key==='ArrowLeft') pan(step,0);
    else if(e.key==='ArrowRight') pan(-step,0);
    else if(e.key==='ArrowUp') pan(0,step);
    else if(e.key==='ArrowDown') pan(0,-step);
    else return;
    e.preventDefault();
  });
  new ResizeObserver(refresh).observe(viewport);
  function deactivate() {
    if(frame) cancelAnimationFrame(frame);
    clearTimeout(settle);frame=0;settle=0;anchor=null;
    for(const id of pointers.keys()) if(viewport.hasPointerCapture(id)) viewport.releasePointerCapture(id);
    pointers.clear();viewport.classList.remove('wm-panning');
  }
  // Overscan keeps pins just outside the clipped map. Only offer pins that
  // can actually be seen when a keyboard user steps through this layer.
  function visibleMarkers() {
    const area=viewport.getBoundingClientRect();
    return [...markerLayer.querySelectorAll('button')].filter(pin=>{
      const r=pin.getBoundingClientRect();
      return r.right>area.left&&r.left<area.right&&r.bottom>area.top&&r.top<area.bottom;
    });
  }
  return {geometry,refresh,fit,center,zoom:()=>scale,deactivate,visibleMarkers};
}

// Bins hold references to original points. Panning at the same zoom reuses
// clusters for overlapping bins; filtering replaces the source array.
const mapClusterCache=new WeakMap();
function clusterMapPoints(points,g,size,selected=null,keepIndividual=false) {
  let cache=mapClusterCache.get(points);
  const cols=Math.max(1,Math.floor(g.w/470)),rows=Math.max(1,Math.floor(g.h/470));
  if(!cache || cache.cols!==cols || cache.rows!==rows) {
    cache={cols,rows,bins:new Map(),tiles:new Map()};
    for(const p of points) {
      const key=Math.floor(p.u*cols)+65536*Math.floor(p.v*rows);
      let bin=cache.bins.get(key);
      if(!bin){bin=[];cache.bins.set(key,bin);}
      bin.push(p);
    }
    mapClusterCache.set(points,cache);
  }
  if(cache.w!==g.w || cache.h!==g.h || cache.size!==size ||
     cache.selected!==selected || cache.individual!==keepIndividual){
    cache.tiles.clear();
    Object.assign(cache,{w:g.w,h:g.h,size,selected,individual:keepIndividual});
  }
  const padX=g.padX+18,padY=g.padY+18;
  const left=-g.x-padX,right=-g.x+g.width+padX,top=-g.y-padY,bottom=-g.y+g.height+padY;
  function tileFor(key,bin) {
    let tile=cache.tiles.get(key);
    if(!tile){tile=clusterMapTile(bin,cache);cache.tiles.set(key,tile);}
    return tile;
  }
  if(g.panComplete) {
    const all=[];
    for(const [key,bin] of cache.bins)all.push(...tileFor(key,bin));
    return all;
  }
  const minCol=Math.floor(left*cols/g.w),maxCol=Math.floor(right*cols/g.w);
  const minRow=Math.floor(top*rows/g.h),maxRow=Math.floor(bottom*rows/g.h);
  const visible=[];
  for(let row=minRow;row<=maxRow;row++)for(let col=minCol;col<=maxCol;col++) {
    const key=col+65536*row,pointsInTile=cache.bins.get(key);
    if(!pointsInTile)continue;
    for(const group of tileFor(key,pointsInTile)) {
      if(group.x>=left&&group.x<=right&&group.y>=top&&group.y<=bottom)visible.push(group);
    }
  }
  return visible;
}
function clusterMapTile(points,{w,h,size,selected,individual}) {
  const cells=new Map(), groups=[];
  for(const point of points) {
    const x=point.u*w, y=point.v*h;
    if(point===selected || individual) {groups.push({x,y,points:[point],category:point.category});continue;}
    const cellX=Math.floor(x/size),cellY=Math.floor(y/size);
    let group=null,nearest=size*size;
    for(let a=cellX-1;a<=cellX+1;a++) for(let b=cellY-1;b<=cellY+1;b++) {
      const candidates=cells.get(a+b*65536);if(!candidates) continue;
      for(const candidate of candidates) {
        const d=(x-candidate.anchorX)**2+(y-candidate.anchorY)**2;
        if(d<nearest){nearest=d;group=candidate;}
      }
    }
    if(group) {group.points.push(point);group.x+=x;group.y+=y;if(group.category!==point.category)group.category=null;}
    else {
      group={anchorX:x,anchorY:y,x,y,points:[point],category:point.category};
      groups.push(group);
      const key=cellX+cellY*65536;
      if(!cells.has(key)) cells.set(key,[]);
      cells.get(key).push(group);
    }
  }
  for(const group of groups){group.x/=group.points.length;group.y/=group.points.length;}
  return groups;
}
