import {createRouter, createWebHistory, RouteRecordRaw} from 'vue-router'
import ConfigList from '@/views/ConfigList.vue'
import ConfigForm from '@/views/ConfigForm.vue'

const routes: Array<RouteRecordRaw> = [
    {
        path: '/',
        name: 'ConfigList',
        component: ConfigList
    },
    {
        path: '/config/new',
        name: 'NewConfig',
        component: ConfigForm,
        props: {isEdit: false}
    },
    {
        path: '/config/edit/:id',
        name: 'EditConfig',
        component: ConfigForm,
        props: true
    },
    {
        path: '/file/:id',
        name: 'FileManager',
        component: () => import('@/views/FileManager.vue'),
        props: true
    }

]

const router = createRouter({
    history: createWebHistory(),
    routes
})

export default router
