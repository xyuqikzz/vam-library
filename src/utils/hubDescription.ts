import createDOMPurify from 'dompurify'

const purifier = createDOMPurify(window)
purifier.addHook('afterSanitizeAttributes', node => {
  if (node.nodeName === 'A') {
    node.setAttribute('target', '_blank')
    node.setAttribute('rel', 'noopener noreferrer')
  }
})

export function formatDescription(description: string): string {
  const html = description.replace(/\r\n|\n/g, '<br/>')
    .replace(/\[b\](.*?)\[\/b\]/gi, '<strong>$1</strong>')
    .replace(/\[i\](.*?)\[\/i\]/gi, '<em>$1</em>')
    .replace(/\[url=(.*?)\](.*?)\[\/url\]/gi, (_, url: string, label: string) => (
      `<a href="${url.replace(/&/g, '&amp;').replace(/"/g, '&quot;')}" class="about-link">${label}</a>`
    ))
  // Sanitize after BBCode expansion, so both remote HTML and generated links
  // cross the same boundary before reaching Vue's v-html.
  return purifier.sanitize(html, {
    USE_PROFILES: { html: true },
    FORBID_TAGS: ['style', 'form', 'input', 'button', 'textarea', 'select', 'option'],
    FORBID_ATTR: ['style'],
  })
}
