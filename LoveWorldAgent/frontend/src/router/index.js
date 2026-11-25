import { createRouter, createWebHashHistory } from 'vue-router'
import Chat from '../views/Chat.vue'
// 桌面端以 file:// 加载构建产物，必须使用 hash 模式；对话页为唯一页面
const routes = [
    { path: '/', name: 'chat', component: Chat },
    { path: '/:pathMatch(.*)*', redirect: '/' },
]
export default createRouter({
    history: createWebHashHistory(),
    routes,
})
