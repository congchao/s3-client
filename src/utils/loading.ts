// utils/loading.ts
import {createVNode, render} from 'vue'
import {Spin} from 'ant-design-vue'

// 存储 loading 节点和容器
let loadingNode: any = null
let loadingContainer: HTMLElement | null = null

/**
 * 显示全局 loading
 * @param text 提示文字
 * @param size 尺寸（small | default | large）
 */
export const showLoading = (text = '加载中...', size = 'large') => {
    // 避免重复创建
    if (loadingContainer) return

    // 创建容器
    loadingContainer = document.createElement('div')
    loadingContainer.style.position = 'fixed'
    loadingContainer.style.top = '0'
    loadingContainer.style.left = '0'
    loadingContainer.style.right = '0'
    loadingContainer.style.bottom = '0'
    loadingContainer.style.background = 'rgba(255, 255, 255, 0.8)'
    loadingContainer.style.display = 'flex'
    loadingContainer.style.justifyContent = 'center'
    loadingContainer.style.alignItems = 'center'
    loadingContainer.style.zIndex = '9999'
    document.body.appendChild(loadingContainer)

    // 创建 Spin 组件
    loadingNode = createVNode(Spin, {
        spinning: true,
        size,
        tip: text
    })
    // 渲染组件到容器
    render(loadingNode, loadingContainer)
}

/**
 * 隐藏全局 loading
 */
export const hideLoading = () => {
    if (loadingContainer && loadingNode) {
        // 卸载组件
        render(null, loadingContainer)
        // 移除容器
        document.body.removeChild(loadingContainer)
        // 重置状态
        loadingContainer = null
        loadingNode = null
    }
}