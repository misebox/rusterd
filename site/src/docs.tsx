import { render } from "solid-js/web";
import Docs from "./pages/Docs";

const root = document.getElementById("app");
if (root) {
  render(() => <Docs />, root);
}
