// Called by the privileged, origin-bound desktop actor after checking that
// this is a user-initiated export of a valid Pal Atlas world backup.
export async function saveDesktopBackup(window,browser,filename,text) {
  try {
    if(!/^(?:Jade 幻兽帕鲁工具箱|帕鲁手册)-[^\\/:*?"<>|\u0000-\u001f]+\.json$/.test(filename) || typeof text!=="string") {
      return {state:"error",text:"无效的备份文件名或内容。"};
    }
    const picker=Cc["@mozilla.org/filepicker;1"].createInstance(Ci.nsIFilePicker);
    picker.init(window.browsingContext||browser.browsingContext,"导出 Jade 工具箱备份",Ci.nsIFilePicker.modeSave);
    picker.appendFilter("JSON 备份","*.json");
    picker.defaultString=filename;
    picker.defaultExtension="json";
    const result=await new Promise(resolve=>picker.open(resolve));
    if(result!==Ci.nsIFilePicker.returnOK && result!==Ci.nsIFilePicker.returnReplace) {
      return {state:"cancelled",text:"已取消保存。"};
    }
    const file=picker.file;
    if(!file?.path || !/\.json$/i.test(file.leafName)) {
      return {state:"error",text:"请重新导出，并选择以 .json 结尾的备份文件名。"};
    }
    // The path comes exclusively from the picker. Replace an existing backup
    // only after its complete new contents have reached a temporary sibling.
    const tmp=file.path+".pal-tmp-"+Services.uuid.generateUUID().toString();
    try {
      await IOUtils.writeUTF8(file.path,text,{tmpPath:tmp});
    } finally {
      try {await IOUtils.remove(tmp,{ignoreAbsent:true});} catch (_) { /* The user may have disconnected this drive. */ }
    }
    return {state:"saved",text:"备份已保存为「"+file.leafName+"」。"};
  } catch (error) {
    console.error("Pal Atlas backup could not be saved",error);
    return {state:"error",text:"备份保存失败，请检查目标目录的写入权限和可用空间后重试。"};
  }
}
