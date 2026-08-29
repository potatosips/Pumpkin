const net = require('net');

const socket = net.createConnection({ host: '127.0.0.1', port: 25565 }, () => {
  console.log('Connected to Pumpkin TCP socket!');

  function writeVarInt(value) {
    const bytes = [];
    do {
      let temp = value & 0x7F;
      value >>>= 7;
      if (value !== 0) {
        temp |= 0x80;
      }
      bytes.push(temp);
    } while (value !== 0);
    return Buffer.from(bytes);
  }

  function writeString(str) {
    const strBuf = Buffer.from(str, 'utf8');
    return Buffer.concat([writeVarInt(strBuf.length), strBuf]);
  }

  function createPacket(packetId, dataBuf) {
    const idBuf = writeVarInt(packetId);
    const totalLen = idBuf.length + dataBuf.length;
    return Buffer.concat([writeVarInt(totalLen), idBuf, dataBuf]);
  }

  // 1. Handshake Packet (ID 0x00, protocol 769 for 1.21.4, nextState 2 for Login)
  const handshakeData = Buffer.concat([
    writeVarInt(769), // 1.21.4
    writeString('127.0.0.1'),
    Buffer.from([0x63, 0xDD]), // port 25565
    writeVarInt(2) // nextState = login
  ]);
  socket.write(createPacket(0x00, handshakeData));

  // 2. Login Start Packet (ID 0x00, name = "potatosips", uuid = 0)
  const uuidBuf = Buffer.alloc(16);
  const loginStartData = Buffer.concat([
    writeString('potatosips'),
    uuidBuf
  ]);
  socket.write(createPacket(0x00, loginStartData));
});

let state = 'login';
let incoming = Buffer.alloc(0);

socket.on('data', (chunk) => {
  incoming = Buffer.concat([incoming, chunk]);

  while (incoming.length > 0) {
    let offset = 0;
    
    function readVarInt() {
      let result = 0;
      let shift = 0;
      let byte;
      do {
        if (offset >= incoming.length) return null;
        byte = incoming[offset++];
        result |= (byte & 0x7F) << shift;
        shift += 7;
      } while ((byte & 0x80) !== 0);
      return result;
    }

    const packetLen = readVarInt();
    if (packetLen === null) break;
    const headerLen = offset;
    if (incoming.length < headerLen + packetLen) {
      // Waiting for full packet
      break;
    }

    const packetData = incoming.slice(headerLen, headerLen + packetLen);
    incoming = incoming.slice(headerLen + packetLen);

    // Parse packet ID
    let pOffset = 0;
    function readPacketVarInt() {
      let result = 0;
      let shift = 0;
      let byte;
      do {
        byte = packetData[pOffset++];
        result |= (byte & 0x7F) << shift;
        shift += 7;
      } while ((byte & 0x80) !== 0);
      return result;
    }

    const packetId = readPacketVarInt();
    console.log(`[STATE ${state}] Got packet ID: 0x${packetId.toString(16)} (len: ${packetLen})`);

    // Handle Login Success (ID 0x02)
    if (state === 'login' && packetId === 0x02) {
      console.log('Login success! Acknowledging login...');
      state = 'config';
      // Send Login Acknowledged (0x03)
      socket.write(createPacket(0x03, Buffer.alloc(0)));
    } else if (state === 'config' && packetId === 0x03) {
      // Finish config (0x03)
      console.log('Finish config received! Acknowledging...');
      state = 'play';
      // Send Finish Configuration Acknowledged (0x03)
      socket.write(createPacket(0x03, Buffer.alloc(0)));
    } else if (state === 'play' && (packetId === 0x11 || packetId === 0x12 || packetId === 0x10)) {
      console.log(`Analyzing play packet 0x${packetId.toString(16)}...`);
    }
  }
});

socket.on('error', (err) => {
  console.error('Socket error:', err.message);
});
