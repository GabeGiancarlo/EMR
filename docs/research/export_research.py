"""Build research reading formats using the bundled Python runtime."""
from pathlib import Path
import re, html, json
from docx import Document
from docx.shared import Inches, Pt, RGBColor
from docx.enum.section import WD_SECTION_START, WD_ORIENT
from docx.enum.table import WD_TABLE_ALIGNMENT, WD_CELL_VERTICAL_ALIGNMENT
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.oxml import OxmlElement
from docx.oxml.ns import qn
from docx.opc.constants import RELATIONSHIP_TYPE as RT

ROOT = Path(__file__).parent
SOURCE = (ROOT / 'nexus-market-and-product-research.md').read_text(encoding='utf-8')
LEDGER = (ROOT / 'source-ledger.md').read_text(encoding='utf-8')
FIGURES = [
 ('kipu-admissions.png', 'Kipu admissions illustration', 'Marketing illustration showing admissions and task concepts rather than production navigation.', 'https://www.kipuhealth.com/products/behavioral-health-crm/'),
 ('kipu-business-development.png', 'Kipu business development illustration', 'Marketing illustration of a pipeline, analytics and search concepts.', 'https://www.kipuhealth.com/products/behavioral-health-crm/'),
 ('myavatar-clinical.png', 'Netsmart myAvatar clinical screen', 'Public application screenshot in device artwork. Dense clinical context and medication timing are visible; live behavior was not tested.', 'https://www.ntst.com/offerings/myavatar'),
 ('myavatar-financial.png', 'Netsmart myAvatar financial screen', 'Public application screenshot in device artwork. Aging and verification work appear on the financial screen.', 'https://www.ntst.com/offerings/myavatar'),
 ('alleva-billing.png', 'Alleva billing home screen', 'Official support screenshot showing aging, review work, statistics and financial navigation.', 'https://support.helloalleva.com/alleva-billing-overview'),
]

def blocks(text):
 lines = text.splitlines(); i = 0
 while i < len(lines):
  s = lines[i].strip()
  if not s: i += 1; continue
  if s.startswith('|'):
   rows=[]
   while i < len(lines) and lines[i].strip().startswith('|'):
    row=[x.strip() for x in lines[i].strip().strip('|').split('|')]
    if not all(re.fullmatch(r'[:\- ]+',x) for x in row): rows.append(row)
    i+=1
   yield ('table', rows); continue
  m=re.match(r'^(#{1,4})\s+(.*)',s)
  if m: yield ('heading',(len(m[1]),m[2]))
  elif re.match(r'^[-*]\s+',s): yield ('bullet',s[2:])
  elif re.match(r'^\d+\.\s+',s): yield ('number',re.sub(r'^\d+\.\s+','',s))
  else: yield ('paragraph',s)
  i+=1

def plain(text):
 text=re.sub(r'\[([^\]]+)\]\([^)]*\)',r'\1',text)
 return text.replace('**','').replace('`','').replace('→',' to ').replace('—',' - ').replace('“','"').replace('”','"')

def heading_text(text):
 return re.sub(r'[^\w\s]',' ',plain(text)).strip()

doc=Document(); section=doc.sections[0]
section.page_width=Inches(8.5);section.page_height=Inches(11)
section.top_margin=Inches(.75);section.bottom_margin=Inches(.7)
section.left_margin=section.right_margin=Inches(.8)
styles=doc.styles
for name in ['Normal','Title','Subtitle','Heading 1','Heading 2','Heading 3','Caption','List Bullet','List Number']:
 st=styles[name];st.font.name='Arial';st.font.color.rgb=RGBColor(0,0,0)
 st.paragraph_format.widow_control=True
styles['Normal'].font.size=Pt(10.5)
styles['Normal'].paragraph_format.line_spacing=1.12
styles['Normal'].paragraph_format.space_after=Pt(7)
styles['Title'].font.size=Pt(29);styles['Title'].font.bold=True
styles['Title'].paragraph_format.space_after=Pt(14)
styles['Heading 1'].font.size=Pt(17);styles['Heading 1'].paragraph_format.space_before=Pt(18)
styles['Heading 2'].font.size=Pt(13);styles['Heading 2'].paragraph_format.space_before=Pt(12)
styles['Heading 3'].font.size=Pt(11);styles['Heading 3'].font.bold=True
styles['Caption'].font.size=Pt(9);styles['Caption'].font.italic=False
for name in ['Heading 1','Heading 2','Heading 3','Title']:
 styles[name].paragraph_format.keep_with_next=True
 ppr=styles[name].element.find(qn('w:pPr'))
 if ppr is not None:
  for border in list(ppr.findall(qn('w:pBdr'))):ppr.remove(border)

def link(p,label,url):
 if not url.startswith('http'): return p.add_run(label)
 h=OxmlElement('w:hyperlink');h.set(qn('r:id'),p.part.relate_to(url,RT.HYPERLINK,is_external=True))
 r=OxmlElement('w:r');pr=OxmlElement('w:rPr');c=OxmlElement('w:color');c.set(qn('w:val'),'155E63');pr.append(c)
 u=OxmlElement('w:u');u.set(qn('w:val'),'single');pr.append(u);r.append(pr)
 t=OxmlElement('w:t');t.text=label;r.append(t);h.append(r);p._p.append(h)

def inline(p,text):
 pattern=r'(\[[^\]]+\]\([^)]*\)|\*\*.*?\*\*|`[^`]+`)'
 for part in re.split(pattern,text):
  m=re.match(r'^\[([^\]]+)\]\(([^)]*)\)$',part)
  if m:link(p,m[1],m[2])
  elif part.startswith('**'):p.add_run(plain(part)).bold=True
  elif part.startswith('`'):p.add_run(plain(part)).italic=True
  else:p.add_run(plain(part))

def table(rows):
 n=len(rows[0]); t=doc.add_table(rows=1,cols=n);t.autofit=False;t.alignment=WD_TABLE_ALIGNMENT.CENTER
 width=9.4 if n>4 else 6.9
 ratios=([1.4,1.2,1.2,1.2,1.2,1.2,2.0] if n==7 else [1.35,2.25,3.3] if n==3 else [1]*n)
 total=sum(ratios)
 for j,col in enumerate(t.columns):col.width=Inches(width*ratios[j]/total)
 for idx,row in enumerate(rows):
  cells=t.rows[0].cells if idx==0 else t.add_row().cells
  for j,(cell,text) in enumerate(zip(cells,row)):
   cell.width=Inches(width*ratios[j]/total);cell.vertical_alignment=WD_CELL_VERTICAL_ALIGNMENT.CENTER
   p=cell.paragraphs[0];p.paragraph_format.space_after=Pt(2);p.paragraph_format.space_before=Pt(2);p.paragraph_format.line_spacing=1.05
   if n==7 and j in range(1,6):p.alignment=WD_ALIGN_PARAGRAPH.CENTER
   inline(p,text)
   for r in p.runs:r.font.size=Pt(8.5 if n>4 else 9.5);r.bold=idx==0;r.font.color.rgb=RGBColor(255,255,255) if idx==0 else RGBColor(0,0,0)
   tcpr=cell._tc.get_or_add_tcPr();shade=OxmlElement('w:shd');shade.set(qn('w:fill'),'193B4B' if idx==0 else ('F2F5F6' if idx%2==0 else 'FFFFFF'));tcpr.append(shade)
   mar=OxmlElement('w:tcMar')
   for side in ['top','left','bottom','right']:
    elem=OxmlElement('w:'+side);elem.set(qn('w:w'),'90');elem.set(qn('w:type'),'dxa');mar.append(elem)
   tcpr.append(mar)
   borders=OxmlElement('w:tcBorders')
   for side in ['top','left','bottom','right','insideH','insideV']:
    elem=OxmlElement('w:'+side);elem.set(qn('w:val'),'single');elem.set(qn('w:sz'),'4');elem.set(qn('w:color'),'D9D9D9');borders.append(elem)
   tcpr.append(borders)
  trpr=t.rows[idx]._tr.get_or_add_trPr();trpr.append(OxmlElement('w:cantSplit'))
  if idx==0:trpr.append(OxmlElement('w:tblHeader'))
 doc.add_paragraph().paragraph_format.space_after=Pt(1)

def orientation(landscape):
 sec=doc.add_section(WD_SECTION_START.NEW_PAGE)
 sec.orientation=WD_ORIENT.LANDSCAPE if landscape else WD_ORIENT.PORTRAIT
 sec.page_width=Inches(11 if landscape else 8.5);sec.page_height=Inches(8.5 if landscape else 11)
 sec.left_margin=sec.right_margin=Inches(.8)

in_matrix=False
for kind,data in blocks(SOURCE):
 if kind=='heading':
  level,text=data
  if text=='Capability comparison':orientation(True);in_matrix=True
  elif in_matrix and level==2:orientation(False);in_matrix=False
  doc.add_paragraph(heading_text(text),'Title' if level==1 else 'Heading '+str(level-1))
 elif kind=='table':table(data)
 else:
  p=doc.add_paragraph(style='List Bullet' if kind=='bullet' else 'List Number' if kind=='number' else 'Normal');inline(p,data)

doc.add_page_break();doc.add_heading('Visual reference appendix',1)
doc.add_paragraph('Publisher assets retrieved November 2, 2026. These examples support comparison of visible information structure. They are not Nexus designs or evidence of a tested product workflow. Copyright and trademarks remain with their owners.')
for i,(filename,title,caption,url) in enumerate(FIGURES):
 if i in [2,4]:doc.add_page_break()
 doc.add_heading(title,2)
 p=doc.add_paragraph();p.paragraph_format.keep_with_next=True
 shape=p.add_run().add_picture(str(ROOT/'snapshots'/filename),width=Inches(3.5 if i<2 else 6.2))
 shape._inline.docPr.set('descr',title+'. '+caption)
 p=doc.add_paragraph(style='Caption');inline(p,caption+' Source: ['+title+']('+url+').')

doc.add_page_break();doc.add_heading('Source ledger',1)
doc.add_paragraph('Primary sources retrieved November 2, 2026. Links are live references; publication and product details may change. Vendor statements establish published capabilities, not independently verified performance.')
ledger_rows=next(data for kind,data in blocks(LEDGER) if kind=='table')
for row in ledger_rows[1:]:
 p=doc.add_paragraph();p.paragraph_format.keep_with_next=True
 p.add_run(row[0]+'  ').bold=True;inline(p,row[1])
 p=doc.add_paragraph();inline(p,'Supports: '+row[2]+'. '+row[3]+'.')
 p.paragraph_format.space_after=Pt(10)

footer=doc.sections[0].footer.paragraphs[0];footer.alignment=WD_ALIGN_PARAGRAPH.RIGHT
r=footer.add_run('Nexus research  |  ');r.font.size=Pt(8)
field=OxmlElement('w:fldSimple');field.set(qn('w:instr'),'PAGE');footer._p.append(field)
doc.core_properties.title='Nexus recovery care market and product research'
doc.core_properties.subject='Competitive evidence and product recommendations for a recovery care EMR'
doc.core_properties.author='Nexus'
out=ROOT/'Nexus-competitive-research.docx';doc.save(out)

def hin(text):
 text=html.escape(text)
 text=text.replace('](source-ledger.md)','](#nexus-research-source-ledger)').replace('](snapshots/README.md)','](#visual-gallery)')
 text=re.sub(r'\[([^\]]+)\]\(([^)]*)\)',lambda m:'<a href="'+m[2]+'">'+m[1]+'</a>',text)
 text=re.sub(r'\*\*(.*?)\*\*',r'<strong>\1</strong>',text)
 text=re.sub(r'`(.*?)`',r'<code>\1</code>',text)
 return text

toc=[];slug_counts={}
def render(text,collect=False):
 out=[];lst=None
 for kind,data in blocks(text):
  if kind not in ['bullet','number'] and lst:out.append('</'+lst+'>');lst=None
  if kind=='heading':
   level,title=data;slug=re.sub(r'[^a-z0-9]+','-',plain(title).lower()).strip('-')
   slug_counts[slug]=slug_counts.get(slug,0)+1
   if slug_counts[slug]>1:slug+='-'+str(slug_counts[slug])
   out.append(f'<h{level} id="{slug}">{hin(title)}</h{level}>')
   if collect and level==2:toc.append((slug,title))
  elif kind=='table':
   out.append('<div class="table-wrap" tabindex="0" aria-label="Scrollable comparison table"><table><thead><tr>'+''.join('<th scope="col">'+hin(x)+'</th>' for x in data[0])+'</tr></thead><tbody>')
   for row in data[1:]:out.append('<tr>'+''.join('<td>'+hin(x)+'</td>' for x in row)+'</tr>')
   out.append('</tbody></table></div>')
  elif kind in ['bullet','number']:
   tag='ul' if kind=='bullet' else 'ol'
   if lst!=tag:
    if lst:out.append('</'+lst+'>')
    out.append('<'+tag+'>');lst=tag
   out.append('<li>'+hin(data)+'</li>')
  else:out.append('<p>'+hin(data)+'</p>')
 if lst:out.append('</'+lst+'>')
 return '\n'.join(out)

body=render(SOURCE,True)
gallery='<section id="visual-gallery"><h2>Visual reference gallery</h2><p>Official publisher assets retrieved November 2, 2026. Copyright and trademarks remain with the publishers. Public examples are not tested tenant workflows.</p>'
for file,title,caption,url in FIGURES:
 gallery+=f'<figure><img src="snapshots/{file}" alt="{html.escape(title)}" loading="lazy"><figcaption><strong>{html.escape(title)}</strong> — {html.escape(caption)} <a href="{html.escape(url)}">Publisher source</a></figcaption></figure>'
gallery+='</section>'
nav=''.join('<a href="#'+slug+'">'+html.escape(title)+'</a>' for slug,title in toc)
css='''*{box-sizing:border-box}html{scroll-behavior:smooth;scroll-padding-top:24px}body{margin:0;background:#f7f5ef;color:#24333b;font:16px/1.65 system-ui,-apple-system,"Segoe UI",sans-serif}a{color:#12656b;text-underline-offset:3px}a:focus-visible,button:focus-visible,.table-wrap:focus-visible{outline:3px solid #c85b43;outline-offset:4px}.layout{display:grid;grid-template-columns:260px minmax(0,1fr);max-width:1480px;margin:auto}aside{position:sticky;top:0;height:100vh;overflow:auto;padding:36px 24px;border-right:1px solid #dedfd9;background:#eef1ec}aside strong{font-size:23px;letter-spacing:2px;color:#132f3d}aside p{font-size:12px;color:#50636a;margin:8px 0 20px}nav a{display:block;text-decoration:none;font-size:13px;line-height:1.4;padding:8px 0}main{padding:48px 58px 70px;max-width:1160px;min-width:0}h1{font-size:clamp(30px,4vw,46px);line-height:1.12;letter-spacing:-1.4px;margin:12px 0 24px;color:#142d39}h2{font-size:27px;line-height:1.25;margin:54px 0 18px;color:#142d39;letter-spacing:-.5px}h3{font-size:20px;margin:30px 0 12px;color:#142d39}p{margin:0 0 17px;max-width:84ch}li{margin:8px 0}code{font-size:.9em;background:#e9ede7;padding:2px 5px;border-radius:4px}.eyebrow{text-transform:uppercase;letter-spacing:2px;font-size:11px;font-weight:700;color:#59736e}.tools{display:flex;align-items:center;justify-content:space-between;gap:16px}button{font:inherit;font-size:13px;background:#193b4b;color:white;border:0;border-radius:7px;padding:9px 15px;cursor:pointer}.table-wrap{overflow:auto;margin:24px 0 30px;background:white;border:1px solid #d9dedb;border-radius:8px}table{border-collapse:collapse;width:100%;font-size:13px;line-height:1.5}th{background:#193b4b;color:white;text-align:left;padding:13px 12px}td{padding:12px;border:1px solid #d9dedb;vertical-align:top;min-width:110px}tr:nth-child(even){background:#f0f4f2}figure{margin:30px 0;padding:22px;background:white;border:1px solid #d9dedb;border-radius:10px;break-inside:avoid}figure img{display:block;max-width:100%;max-height:560px;width:auto;height:auto;margin:auto}figcaption{font-size:13px;color:#4c6066;line-height:1.5;margin-top:18px}.source-ledger{margin-top:55px}footer{border-top:1px solid #d9dedb;margin-top:40px;padding-top:20px;color:#5c6b70;font-size:13px}.skip{position:absolute;left:-10000px}.skip:focus{left:12px;top:12px;background:white;padding:10px;z-index:3}@media(max-width:1000px){.layout{display:block}aside{position:static;height:auto;padding:20px 24px;border:0}nav{display:none}aside p{margin-bottom:0}main{padding:28px 24px}h1{letter-spacing:-.8px}.table-wrap table{min-width:700px}}@media print{body{background:white;color:black;font:10pt/1.4 Arial,sans-serif}.layout{display:block}aside,.tools button,.skip{display:none}main{max-width:none;padding:0}.tools{margin-bottom:8pt}h1{font-size:25pt}h2{font-size:16pt;margin-top:20pt;break-after:avoid}h3{font-size:12pt;break-after:avoid}a{color:inherit}p,li{orphans:3;widows:3}.table-wrap{overflow:visible;border-radius:0;break-inside:auto}table{font-size:8pt}td,th{padding:5pt;min-width:0}thead{display:table-header-group}tr{break-inside:avoid}figure{border:0;padding:0;margin:15pt 0}figure img{max-height:4in}footer{font-size:8pt}@page{size:A4;margin:18mm}}'''
html_doc='<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Nexus recovery care market and product research</title><style>'+css+'</style></head><body><a class="skip" href="#report">Skip to research</a><div class="layout"><aside><strong>NEXUS</strong><p>COMPETITIVE RESEARCH<br>November 2, 2026</p><nav aria-label="Report contents">'+nav+'<a href="#visual-gallery">Visual reference gallery</a><a href="#nexus-research-source-ledger">Source ledger</a></nav></aside><main id="report"><div class="tools"><span class="eyebrow">Recovery care · Product strategy</span><button onclick="window.print()">Print report</button></div>'+body+gallery+'<section class="source-ledger">'+render(LEDGER)+'</section><footer>Nexus research · Primary sources retrieved November 2, 2026 · Proposals require implementation and validation.</footer></main></div></body></html>'
(ROOT/'index.html').write_text(html_doc,encoding='utf-8')
print(json.dumps({'docx':str(out),'html':str(ROOT/'index.html'),'paragraphs':len(doc.paragraphs),'tables':len(doc.tables),'images':len(doc.inline_shapes)}))
