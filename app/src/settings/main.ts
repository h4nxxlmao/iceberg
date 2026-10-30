import { mount } from "svelte";
import Settings from "./Settings.svelte";
import "./settings.css";

const target = document.getElementById("app");
if (target) {
  mount(Settings, { target });
}
