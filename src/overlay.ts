import "./app.css";
import { mount } from "svelte";
import Overlay from "./Overlay.svelte";

window.addEventListener("contextmenu", (e) => e.preventDefault());

export default mount(Overlay, { target: document.getElementById("overlay")! });
