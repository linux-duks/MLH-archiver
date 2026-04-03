const Server       = require('nntp-server');
const join         = require('path').join;
const mock_db      = require('./helpers').mock_db;

const port = 8119;
const addr = `nntp://0.0.0.0:${port}`;

const nntp = new Server(addr, { requireAuth: false });

console.log(`loading mock database`)
mock_db(nntp, join(__dirname, './fixtures/db.yml'));

console.log(`Starting server on ${addr}`)
return nntp.listen(addr).then(() => {
      running_address = nntp.server.address();
			console.log(`Serving on port :${running_address.port}`)
    });
console.log(`Stopped server`)
