const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('saiDesktop', {
  getState: () => ipcRenderer.invoke('desktop:state'),
  perform: (action) => ipcRenderer.invoke('desktop:action', action),
  onState: (callback) => {
    const listener = (_event, value) => callback(value);
    ipcRenderer.on('desktop:state-changed', listener);
    return () => ipcRenderer.removeListener('desktop:state-changed', listener);
  },
});
