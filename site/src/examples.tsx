import { render } from "solid-js/web";
import Examples from "./pages/Examples";

const root = document.getElementById("app");
if (root) {
  render(() => <Examples />, root);
}
