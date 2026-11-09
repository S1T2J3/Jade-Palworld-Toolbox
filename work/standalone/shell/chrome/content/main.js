"use strict";
const themePref="palatlas.ui.theme";
const themes=new Set(["moon","jade","blue","rose","amber","slate"]);
function setTitlebarTheme(theme) {
  if(!themes.has(theme))return;
  document.documentElement.dataset.theme=theme;
  if(Services.prefs.getStringPref(themePref,"moon")!==theme)
    Services.prefs.setStringPref(themePref,theme);
}
// The browser has just been parsed. Supply its initial file before Gecko
// connects the remote element at DOMContentLoaded.
const browser=document.getElementById("atlas");
const html=Cc["@mozilla.org/file/local;1"].createInstance(Ci.nsIFile);
html.initWithPath(Services.env.get("PAL_ATLAS_PAGE"));
browser.setAttribute("src",Services.io.newFileURI(html).spec);
setTitlebarTheme(Services.prefs.getStringPref(themePref,"moon"));
window.addEventListener("DOMContentLoaded",() => {
  const icon=Services.dirsvc.get("GreD",Ci.nsIFile);
  for(const part of ["chrome","icons","default","palatlas-main.ico"])icon.append(part);
  document.getElementById("window-icon").src=Services.io.newFileURI(icon).spec;
  const maximize=document.getElementById("window-maximize");
  const refresh=()=>{
    const label=window.windowState===window.STATE_MAXIMIZED?"还原":"最大化";
    maximize.setAttribute("aria-label",label);maximize.title=label;
  };
  document.getElementById("window-minimize").addEventListener("click",()=>window.minimize());
  maximize.addEventListener("click",()=>{
    if(window.windowState===window.STATE_MAXIMIZED)window.restore();else window.maximize();
  });
  document.getElementById("window-close").addEventListener("click",()=>window.close());
  window.addEventListener("sizemodechange",refresh);
  refresh();
  browser.focus();
},{once:true});
