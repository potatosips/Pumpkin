const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username: 'TestBot',
  version: '1.21.4',
  auth: 'offline'
});

client.on('raw', (buffer, meta) => {
  if (meta.name === 'declare_commands') {
    console.log('--- FOUND DECLARE_COMMANDS PACKET --- length:', buffer.length);
    let offset = 0;
    
    function readVarInt() {
      let result = 0;
      let shift = 0;
      let byte;
      do {
        if (offset >= buffer.length) throw new Error(`Buffer overrun while reading VarInt at offset ${offset}`);
        byte = buffer[offset++];
        result |= (byte & 0x7F) << shift;
        shift += 7;
      } while ((byte & 0x80) !== 0);
      return result;
    }

    function readString() {
      const len = readVarInt();
      const str = buffer.toString('utf8', offset, offset + len);
      offset += len;
      return str;
    }

    try {
      // Packet ID
      const packetId = readVarInt();
      console.log(`Packet ID: 0x${packetId.toString(16)}`);
      
      const nodeCount = readVarInt();
      console.log(`Total nodes: ${nodeCount}`);

      for (let i = 0; i < nodeCount; i++) {
        const startOffset = offset;
        const flags = buffer[offset++];
        const nodeType = flags & 0x03;
        const isExecutable = (flags & 0x04) !== 0;
        const hasRedirect = (flags & 0x08) !== 0;
        const hasSuggestions = (flags & 0x10) !== 0;
        const isRestricted = (flags & 0x20) !== 0;

        const childrenCount = readVarInt();
        const children = [];
        for (let c = 0; c < childrenCount; c++) {
          children.push(readVarInt());
        }

        let redirectTarget = null;
        if (hasRedirect) {
          redirectTarget = readVarInt();
        }

        let name = null;
        if (nodeType === 1 || nodeType === 2) {
          name = readString();
        }

        let parserId = null;
        let parserProp = null;
        if (nodeType === 2) {
          parserId = readVarInt();
          // specific parser properties depending on parserId in 1.21.4
          // e.g. float(1), double(2), integer(3), long(4), string(5), entity(6), score_holder(32), time(44), resource(48), etc.
        }

        let suggestionType = null;
        if (hasSuggestions) {
          suggestionType = readString();
        }

        if (i < 10 || i > nodeCount - 10) {
          console.log(`Node #${i} [offset ${startOffset}]: type=${nodeType} flags=0x${flags.toString(16)} name="${name}" parserId=${parserId} children=[${children}] redirect=${redirectTarget}`);
        }
      }

      const rootIndex = readVarInt();
      console.log(`SUCCESS! Root index: ${rootIndex}, Remaining bytes: ${buffer.length - offset}`);
    } catch (e) {
      console.error(`Decoding failed at offset ${offset}:`, e.message);
    }

    client.end();
    process.exit(0);
  }
});

client.on('error', (err) => {
  console.error('Client error:', err.message);
});
