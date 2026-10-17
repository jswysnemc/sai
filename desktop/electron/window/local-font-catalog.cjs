const { ipcMain } = require('electron');
const { isInternal } = require('../navigation.cjs');
const { listInstalledFontFamilies } = require('../fonts.cjs');

/**
 * 【桌面端】【字体目录】只允许工作台页面读取本机字族。
 * @param {WebContents} workbench Sai 页面
 * @param {string} origin Sai 后端源地址
 * @returns {Function} 窗口关闭时移除接口
 */
function installLocalFontCatalog(workbench, origin) {
  const owned = (event) => event.sender === workbench && event.senderFrame === workbench.mainFrame
    && isInternal(event.senderFrame.url, origin);
  ipcMain.handle('desktop:fonts', async (event) => {
    if (!owned(event)) throw new Error('字体列表只允许工作台调用');
    return listInstalledFontFamilies();
  });
  return () => ipcMain.removeHandler('desktop:fonts');
}

module.exports = { installLocalFontCatalog };
