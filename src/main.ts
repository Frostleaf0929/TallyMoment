import { createApp } from "vue";
import App from "./App.vue";
import "./styles/theme.css";

const app = createApp(App);
// 渲染错误写入窗口标题（调试通道；稳定后可移除）
app.config.errorHandler = (err: unknown) => {
  const msg = err instanceof Error ? `${err.message}` : String(err);
  document.title = "E:" + msg.slice(0, 150);
  console.error(err);
};
app.mount("#app");
