// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="introduction.html"><strong aria-hidden="true">1.</strong> 简介</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="installation.html"><strong aria-hidden="true">1.1.</strong> 安装</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="basic-usage.html"><strong aria-hidden="true">1.2.</strong> 基本用法</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="specifications.html"><strong aria-hidden="true">2.</strong> 规约语言</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="notations.html"><strong aria-hidden="true">2.1.</strong> 词法规则</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="expressions.html"><strong aria-hidden="true">2.2.</strong> 逻辑表达式</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="quantifiers.html"><strong aria-hidden="true">2.2.1.</strong> 量词</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="implication.html"><strong aria-hidden="true">2.2.2.</strong> 蕴含</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="old.html"><strong aria-hidden="true">2.2.3.</strong> 旧状态表达式</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="function-contracts.html"><strong aria-hidden="true">2.3.</strong> 函数契约</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="preconditions.html"><strong aria-hidden="true">2.3.1.</strong> 前置条件</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="postconditions.html"><strong aria-hidden="true">2.3.2.</strong> 后置条件</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="statements.html"><strong aria-hidden="true">2.4.</strong> 语句标注</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="assertions.html"><strong aria-hidden="true">2.4.1.</strong> 断言</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="assumptions.html"><strong aria-hidden="true">2.4.2.</strong> 假设</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="invariants.html"><strong aria-hidden="true">2.5.</strong> 类型不变式</a></span></li></ol><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="grammar.html"><strong aria-hidden="true">3.</strong> 文法索引</a></span></li><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="constraints.html"><strong aria-hidden="true">4.</strong> 实现说明</a></span><ol class="section"><li class="chapter-item expanded "><span class="chapter-link-wrapper"><a href="sources.html"><strong aria-hidden="true">4.1.</strong> 实现依据与测试覆盖</a></span></li></ol></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split('#')[0].split('?')[0];
        if (current_page.endsWith('/')) {
            current_page += 'index.html';
        }
        const links = Array.prototype.slice.call(this.querySelectorAll('a'));
        const l = links.length;
        for (let i = 0; i < l; ++i) {
            const link = links[i];
            const href = link.getAttribute('href');
            if (href && !href.startsWith('#') && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The 'index' page is supposed to alias the first chapter in the book.
            // Check both with and without the '.html' suffix to be robust against pretty URLs
            if (link.href.replace(/\.html$/, '') === current_page.replace(/\.html$/, '')
                || i === 0
                && path_to_root === ''
                && current_page.endsWith('/index.html')) {
                link.classList.add('active');
                let parent = link.parentElement;
                while (parent) {
                    if (parent.tagName === 'LI' && parent.classList.contains('chapter-item')) {
                        parent.classList.add('expanded');
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', e => {
            if (e.target.tagName === 'A') {
                const clientRect = e.target.getBoundingClientRect();
                const sidebarRect = this.getBoundingClientRect();
                sessionStorage.setItem('sidebar-scroll-offset', clientRect.top - sidebarRect.top);
            }
        }, { passive: true });
        const sidebarScrollOffset = sessionStorage.getItem('sidebar-scroll-offset');
        sessionStorage.removeItem('sidebar-scroll-offset');
        if (sidebarScrollOffset !== null) {
            // preserve sidebar scroll position when navigating via links within sidebar
            const activeSection = this.querySelector('.active');
            if (activeSection) {
                const clientRect = activeSection.getBoundingClientRect();
                const sidebarRect = this.getBoundingClientRect();
                const currentOffset = clientRect.top - sidebarRect.top;
                this.scrollTop += currentOffset - parseFloat(sidebarScrollOffset);
            }
        } else {
            // scroll sidebar to current active section when navigating via
            // 'next/previous chapter' buttons
            const activeSection = document.querySelector('#mdbook-sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        const sidebarAnchorToggles = document.querySelectorAll('.chapter-fold-toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(el => {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define('mdbook-sidebar-scrollbox', MDBookSidebarScrollbox);

