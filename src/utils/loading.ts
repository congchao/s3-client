// utils/loading.ts
import {createVNode, render} from 'vue'
import {Spin} from 'ant-design-vue'

// 存储 loading 节点和容器
let loadingNode: any = null
let loadingContainer: HTMLElement | null = null
let loadingMount: HTMLElement | null = null
let loadingId = 0

interface LoadingOptions {
    cancelText?: string
    onCancel?: () => void
}

export interface LoadingControl {
    id: number
    cancelled: boolean
    close: () => void
}

/**
 * 显示全局 loading
 * @param text 提示文字
 * @param size 尺寸（small | default | large）
 */
export const showLoading = (
    text = '加载中...',
    size = 'large',
    options: LoadingOptions = {}
): LoadingControl => {
    // 避免重复创建
    if (loadingContainer) {
        hideLoading()
    }

    const currentId = ++loadingId
    let cancelled = false

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

    const content = document.createElement('div')
    content.style.display = 'flex'
    content.style.flexDirection = 'column'
    content.style.alignItems = 'center'
    content.style.gap = '16px'
    loadingContainer.appendChild(content)

    loadingMount = document.createElement('div')
    content.appendChild(loadingMount)

    // 创建 Spin 组件
    loadingNode = createVNode(Spin, {
        spinning: true,
        size,
        tip: text
    })
    // 渲染组件到容器
    render(loadingNode, loadingMount)

    if (options.onCancel) {
        const cancelButton = document.createElement('button')
        cancelButton.type = 'button'
        cancelButton.textContent = options.cancelText || '取消'
        cancelButton.style.border = '1px solid #d9d9d9'
        cancelButton.style.background = '#fff'
        cancelButton.style.borderRadius = '6px'
        cancelButton.style.padding = '4px 14px'
        cancelButton.style.cursor = 'pointer'
        cancelButton.style.color = '#333'
        cancelButton.onclick = () => {
            cancelled = true
            options.onCancel?.()
            closeLoading(currentId)
        }
        content.appendChild(cancelButton)
    }

    return {
        id: currentId,
        get cancelled() {
            return cancelled
        },
        close: () => closeLoading(currentId),
    }
}

const closeLoading = (id?: number) => {
    if (id && id !== loadingId) return
    if (loadingContainer && loadingNode && loadingMount) {
        render(null, loadingMount)
        document.body.removeChild(loadingContainer)
        loadingContainer = null
        loadingMount = null
        loadingNode = null
    }
}

/**
 * 隐藏全局 loading
 */
export const hideLoading = () => {
    closeLoading()
}
