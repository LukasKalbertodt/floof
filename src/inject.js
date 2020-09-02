// This code is inserted automatically by floof. It's here to enable
// autoreloading in the browser. It simply works by opening a websocket
// connection and reloading once the connection closes.
const ws_backend_addr = 'ws://localhost:INSERT_PORT_HERE_KTHXBYE';

function reload() {
    console.log("Received refresh request from floof: reloading page...");
    location.reload();
}
function connectionError() {
    console.warn(`floof could not connect to web socket backend ${ws_backend_addr} :(`);
}

const socket = new WebSocket(ws_backend_addr);

// The actual "socket closed" -> "reload" handler is only installed after the
// connection is successfully established.
socket.addEventListener("close", connectionError);
socket.addEventListener("open", () => {
    socket.removeEventListener("close", connectionError)
    socket.addEventListener("close", reload);
});
