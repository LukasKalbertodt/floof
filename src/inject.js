const socket = new WebSocket('ws://localhost:INSERT_PORT_HERE_KTHXBYE');
socket.addEventListener("close", () => {
    console.log("Received refresh request from watchboi: reloading page...");
    location.reload();
});
