import { render } from "solid-js/web";
import Demo from "./pages/Demo";

const root = document.getElementById("app");
if (root) {
  render(() => <Demo />, root);
}
