import { createApp } from 'vue'
import './assets/tailwind.css'
import App from './App.vue'
import router from './router'
import { initTheme } from './theme'
createApp(App).use(router).mount('#app')
initTheme()
