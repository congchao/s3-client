import {createApp} from 'vue'
import App from './App.vue'
import 'ant-design-vue/dist/reset.css';
import '@/styles/style.less'
import Antd from 'ant-design-vue';
import {createPinia} from 'pinia'
import piniaPersist from 'pinia-plugin-persist-uni'
import router from '@/router/index'
import SvgIcon from '@/components/Svg.vue'
import 'virtual:svg-icons-register'

const pinia = createPinia()
pinia.use(piniaPersist)
let app = createApp(App)
app.component('svg-icon', SvgIcon)
app.use(pinia)
app.use(Antd)
app.use(router)
app.mount('#app')