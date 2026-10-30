import { mount } from "svelte";
import Widget from "./Widget.svelte";
import "./widget.css";

const target = document.getElementById("app");
if (target) {
  mount(Widget, { target });
}
