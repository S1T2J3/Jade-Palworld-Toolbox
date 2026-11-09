// Translate the original light, green UI palette into shared roles at build
// time. Game elements, map legends, errors and other data colours stay literal.
// No CSS parsing or palette generation is needed while opening a page.
const gameColours = /(?:\[data-(?:rank|element|negative|sex|cg-result|error)|\.pd-(?:neutral|fire|water|electric|grass|ice|dark|ground|dragon|rank-)\b|\.pd-rank-|\.pair-sex|\.wh-sex|\.cl-sex|\.sex-note|\.status-dot|\.wm-done-tag|\.sm-site-mark|\.sm-boss-count|\.cg-(?:diagram|flow)|#(?:wm|sm)-(?:viewport|markers|sheet|coordinate-target)|\.wm-(?:custom-art|game-art|kind-icon|symbol|pin-total)|\.(?:world-danger|cc-error)|#(?:wm-pin-error)|\[data-cl-remove\])/;
const roleProperties = /^(?:color|background(?:-color)?|border(?:-[a-z]+){0,3}|outline(?:-color)?|accent-color|scrollbar-color|--ic-(?:green|ink|secondary))$/;
const neutralPaper = /^#(?:fff|ffffff|fffefa|fffef9|fffefb|fffdf[0-9a-f]|fffef[0-9a-f]|fffcf[4-9a-f])$/i;

function role(hex, property) {
  let digits=hex.slice(1);
  if(digits.length<5)digits=[...digits].map(c=>c+c).join('');
  const literal='#'+digits.slice(0,6);
  const [r,g,b]=[0,2,4].map(i=>parseInt(digits.slice(i,i+2),16));
  const value=(r+2*g+b)/4, chromeGreen=g>=r&&g>b&&g-b<=110;
  if(!chromeGreen&&!neutralPaper.test(literal))return hex;

  let token;
  if(property==='color'||property==='--ic-ink'||property==='--ic-green'||property==='--ic-secondary'){
    if(value>175)token='--skin-on-accent';
    else if(value<64)token='--ink';
    else if(g-b>24||g-r>34)token='--skin-accent';
    else if(value<85)token='--skin-deep';
    else if(value<108)token='--soft';
    else token='--muted';
  }else if(property.startsWith('background')){
    if(value<122)token='--skin-accent';
    else if(value<173)token='--skin-focus';
    else if(value<225)token='--skin-selected';
    else if(value<238)token='--skin-tint';
    else if(value<249)token='--skin-surface-soft';
    else token='--paper';
  }else if(property==='accent-color'||property.startsWith('outline')){
    token='--skin-focus';
  }else if(value<125)token='--skin-accent';
  else if(value<183)token='--skin-edge-strong';
  else if(value<224)token='--border';
  else token='--line';

  if(digits.length===8){
    const opacity=Math.round(parseInt(digits.slice(6),16)/255*100);
    return 'color-mix(in srgb,var('+token+') '+opacity+'%,transparent)';
  }
  return 'var('+token+')';
}

function themedDeclarations(declarations) {
  return declarations.replace(/(^|;)([\s\w-]+)\s*:([^;]*)/g,(chunk,sep,name,original)=>{
    const property=name.trim().toLowerCase();
    if(!roleProperties.test(property))return chunk;
    // Keep token fallbacks (including Pal tint and its outline) and assets intact.
    const value=original.replace(/url\([^)]*\)|var\([^)]*\)|#[\da-f]{3,8}\b/gi,
      match=>match[0]==='#'?role(match,property):match);
    return sep+name+':'+value;
  });
}

module.exports=function themeStyles(css) {
  let selector='',previous='';
  return css.replace(/([^{}]*)([{}])/g,(chunk,part,brace)=>{
    let result=part;
    if(brace==='{')selector=part.replace(/\/\*[\s\S]*?\*\//g,'').trim();
    else if(previous==='{'&&selector!==':root'&&!gameColours.test(selector))result=themedDeclarations(part);
    previous=brace;
    return result+brace;
  });
};
