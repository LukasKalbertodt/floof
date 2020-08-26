// This code is inserted automatically by floof. It's here to enable
// autoreloading in the browser. It simply works by opening a websocket
// connection and reloading once the connection closes.
const socket = new WebSocket('ws://localhost:INSERT_PORT_HERE_KTHXBYE');
socket.addEventListener("close", () => {
    console.log("Received refresh request from floof: reloading page...");
    location.reload();
});
