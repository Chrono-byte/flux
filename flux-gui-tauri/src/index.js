import init, { mount_app } from "./pkg/flux_gui.js";

async fn run() {
    await init();
    mount_app();
}

run();
