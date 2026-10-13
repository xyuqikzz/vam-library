import { computed, onBeforeUnmount, ref, watch, type Ref } from 'vue'

/**
 * 全屏看图的缩放 / 平移。
 * 缩放比例相对于图片原始像素：1 = 100%。「适应屏幕」会把小图放大、大图缩小到刚好放进看图区，
 * 所以 512px 的预览图在全屏下也能真正放大查看。
 */
export function useImageZoom(stage: Ref<HTMLElement | null>) {
  const natural = ref({ width: 0, height: 0 })
  const box = ref({ width: 0, height: 0 })
  const scale = ref(1)
  const x = ref(0)
  const y = ref(0)
  const fitted = ref(true)
  const dragging = ref(false)

  const fitScale = computed(() => {
    const { width, height } = natural.value
    if (!width || !height || !box.value.width || !box.value.height) return 1
    return Math.min(box.value.width / width, box.value.height / height)
  })
  const minScale = computed(() => Math.min(fitScale.value, 1) / 2)
  const maxScale = computed(() => Math.max(fitScale.value, 1) * 8)
  const ready = computed(() => natural.value.width > 0)
  const percent = computed(() => Math.round(scale.value * 100))
  const canZoomIn = computed(() => ready.value && scale.value < maxScale.value - 1e-6)
  const canZoomOut = computed(() => ready.value && scale.value > minScale.value + 1e-6)
  const canPan = computed(() => natural.value.width * scale.value > box.value.width + 1
    || natural.value.height * scale.value > box.value.height + 1)
  const imageStyle = computed(() => ready.value
    ? {
        width: `${natural.value.width}px`,
        height: `${natural.value.height}px`,
        transform: `translate(-50%, -50%) translate(${x.value}px, ${y.value}px) scale(${scale.value})`,
      }
    : { visibility: 'hidden' as const })

  function clampPan() {
    const overflowX = Math.max(0, (natural.value.width * scale.value - box.value.width) / 2)
    const overflowY = Math.max(0, (natural.value.height * scale.value - box.value.height) / 2)
    x.value = Math.min(overflowX, Math.max(-overflowX, x.value))
    y.value = Math.min(overflowY, Math.max(-overflowY, y.value))
  }
  function fit() {
    scale.value = fitScale.value
    x.value = 0
    y.value = 0
    fitted.value = true
  }
  /** origin：缩放中心，相对看图区中心的像素坐标；不传就以中心缩放 */
  function zoomTo(next: number, origin = { x: 0, y: 0 }) {
    if (!ready.value) return
    const target = Math.min(maxScale.value, Math.max(minScale.value, next))
    const ratio = target / scale.value
    x.value = origin.x - (origin.x - x.value) * ratio
    y.value = origin.y - (origin.y - y.value) * ratio
    scale.value = target
    fitted.value = Math.abs(target - fitScale.value) < 1e-3
    clampPan()
  }
  const zoomIn = () => zoomTo(scale.value * 1.25)
  const zoomOut = () => zoomTo(scale.value / 1.25)
  const actualSize = () => zoomTo(1)

  function measureBox() {
    const el = stage.value
    if (!el) return
    const style = getComputedStyle(el)
    box.value = {
      width: Math.max(0, el.clientWidth - parseFloat(style.paddingLeft || '0') - parseFloat(style.paddingRight || '0')),
      height: Math.max(0, el.clientHeight - parseFloat(style.paddingTop || '0') - parseFloat(style.paddingBottom || '0')),
    }
  }
  function loaded(event: Event) {
    const img = event.target as HTMLImageElement
    measureBox()
    natural.value = { width: img.naturalWidth, height: img.naturalHeight }
    fit()
  }
  function reset() {
    natural.value = { width: 0, height: 0 }
    scale.value = 1
    x.value = 0
    y.value = 0
    fitted.value = true
  }
  // 窗口大小变了：适应屏幕状态下重新适应，放大状态下只修正平移范围
  watch(box, () => { if (fitted.value) fit(); else clampPan() })
  let observer: ResizeObserver | null = null
  watch(stage, (el, _previous, onCleanup) => {
    observer?.disconnect()
    if (!el) return
    measureBox()
    if (typeof ResizeObserver !== 'undefined') {
      observer = new ResizeObserver(measureBox)
      observer.observe(el)
    } else {
      window.addEventListener('resize', measureBox)
      onCleanup(() => window.removeEventListener('resize', measureBox))
    }
  })
  onBeforeUnmount(() => observer?.disconnect())

  function originOf(event: MouseEvent) {
    const rect = stage.value?.getBoundingClientRect()
    if (!rect) return { x: 0, y: 0 }
    return { x: event.clientX - rect.left - rect.width / 2, y: event.clientY - rect.top - rect.height / 2 }
  }
  function wheel(event: WheelEvent) {
    if (!ready.value) return
    event.preventDefault()
    // 触控板的捏合缩放 deltaY 很小，鼠标滚轮一格约 100，这样两者手感都比较自然
    zoomTo(scale.value * Math.exp(-event.deltaY * 0.0015), originOf(event))
  }
  /** 双击：适应屏幕时放大到 100%（图本来就小的话放大到 2 倍），否则回到适应屏幕 */
  function toggle(event: MouseEvent) {
    if (!ready.value) return
    if (fitted.value) zoomTo(fitScale.value < 0.95 ? 1 : fitScale.value * 2, originOf(event))
    else fit()
  }
  function startDrag(event: PointerEvent) {
    if (event.button !== 0 || !canPan.value) return
    event.preventDefault()
    const target = event.currentTarget as HTMLElement
    target.setPointerCapture?.(event.pointerId)
    const start = { x: event.clientX - x.value, y: event.clientY - y.value }
    dragging.value = true
    const move = (e: PointerEvent) => {
      x.value = e.clientX - start.x
      y.value = e.clientY - start.y
      clampPan()
    }
    const end = () => {
      dragging.value = false
      target.removeEventListener('pointermove', move)
      target.removeEventListener('pointerup', end)
      target.removeEventListener('pointercancel', end)
    }
    target.addEventListener('pointermove', move)
    target.addEventListener('pointerup', end)
    target.addEventListener('pointercancel', end)
  }

  return {
    scale, percent, fitted, dragging, canPan, canZoomIn, canZoomOut, ready, imageStyle,
    fit, zoomIn, zoomOut, actualSize, zoomTo, wheel, toggle, startDrag, loaded, reset,
  }
}
