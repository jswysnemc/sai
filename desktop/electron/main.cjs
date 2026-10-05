// 【桌面端】【进程入口】浏览器进程不进入桌面窗口及单实例逻辑
if (process.argv.includes('--sai-browser-host')) {
  require('./browser-host/main.cjs').startBrowserHost().catch((error) => {
    process.stderr.write(`【桌面浏览器】【启动失败】${error.stack || error}\n`);
    require('electron').app.exit(1);
  });
} else {
  require('./application.cjs');
}
