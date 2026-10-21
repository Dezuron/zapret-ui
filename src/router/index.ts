import { createRouter, createWebHashHistory } from "vue-router";
import OverviewPage from "../pages/OverviewPage.vue";
import SettingsPage from "../pages/SettingsPage.vue";
import TelegramPage from "../pages/TelegramPage.vue";
import ZapretPage from "../pages/ZapretPage.vue";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "overview", component: OverviewPage, meta: { title: "Главная" } },
    { path: "/zapret", name: "zapret", component: ZapretPage, meta: { title: "Zapret" } },
    { path: "/telegram", name: "telegram", component: TelegramPage, meta: { title: "Telegram" } },
    { path: "/settings", name: "settings", component: SettingsPage, meta: { title: "Настройки" } },
    { path: "/:pathMatch(.*)*", redirect: "/" },
  ],
});

export default router;
