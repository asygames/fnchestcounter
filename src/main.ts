import "./app.css";
import { mount } from "svelte";
import App from "./App.svelte";

// Fără meniul contextual al browserului (aplicația are propriile butoane).
window.addEventListener("contextmenu", (e) => {
  if (!(e.target instanceof HTMLInputElement)) e.preventDefault();
});

export default mount(App, { target: document.getElementById("app")! });
