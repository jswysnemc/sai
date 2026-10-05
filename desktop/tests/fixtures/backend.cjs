const http = require('node:http');
const mode = process.env.SAI_TEST_MODE;
if (mode === 'exit') process.exit(17);
if (mode === 'hang') {
  setInterval(() => {}, 1000);
} else {
  const server = http.createServer((_request, response) => {
    response.setHeader('Content-Type', 'application/json');
    response.end(JSON.stringify({ ok: mode !== 'unhealthy', version: '0.2.4' }));
  });
  const port = Number(process.argv[process.argv.indexOf('--port') + 1]);
  server.listen(port, '127.0.0.1', () => {
    const line = `Sai Web: http://127.0.0.1:${server.address().port}/?token=test-secret_token`;
    // 1. 【桌面测试】【分段输出】模拟启动行跨多个管道数据块到达
    process.stdout.write(line.slice(0, 12));
    setTimeout(() => process.stdout.write(`${line.slice(12)}\n`), 20);
    if (mode === 'crash') setTimeout(() => process.exit(18), 700);
  });
  process.on('SIGTERM', () => server.close(() => process.exit(0)));
}
