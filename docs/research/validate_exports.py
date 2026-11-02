"""Structural checks do not replace visual layout review."""
from pathlib import Path
from html.parser import HTMLParser
from zipfile import ZipFile
import xml.etree.ElementTree as ET
import json

root=Path(__file__).parent
class ReportParser(HTMLParser):
 def __init__(self):super().__init__();self.ids=[];self.links=[];self.images=[]
 def handle_starttag(self,tag,attrs):
  a=dict(attrs)
  if 'id' in a:self.ids.append(a['id'])
  if tag=='a':self.links.append(a.get('href',''))
  if tag=='img':self.images.append(a)
p=ReportParser();p.feed((root/'index.html').read_text(encoding='utf-8'))
assert len(p.ids)==len(set(p.ids)), 'duplicate anchors'
assert all(x[1:] in p.ids for x in p.links if x.startswith('#')), 'broken anchor'
assert all((root/x['src']).exists() and x.get('alt') for x in p.images), 'missing image or alt'
with ZipFile(root/'Nexus-competitive-research.docx') as z:
 document=ET.fromstring(z.read('word/document.xml'))
 rels=ET.fromstring(z.read('word/_rels/document.xml.rels'))
 ns={'w':'http://schemas.openxmlformats.org/wordprocessingml/2006/main','wp':'http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing'}
 assert len(document.findall('.//wp:inline',ns))==5
 assert all(x.get('descr') for x in document.findall('.//wp:docPr',ns))
 tables=document.findall('.//w:tbl',ns)
 assert len(tables)==3
 assert all(t.find('.//w:tblHeader',ns) is not None for t in tables)
 hyperlinks=[x for x in rels if x.get('Type','').endswith('hyperlink')]
 assert len(hyperlinks)>=25
 doc_text=' '.join(x.text or '' for x in document.findall('.//w:t',ns))
 assert all(x in doc_text for x in ['QuickBooks','Part 2','Interview guide','Data model recommendations','Source ledger'])
 result={'html_anchors':len(p.ids),'html_images':len(p.images),'docx_tables':len(tables),'docx_images':5,'docx_external_links':len(hyperlinks),'structural_checks':'passed','docx_visual_review':'blocked: no bundled LibreOffice on Windows','html_visual_review':'pending parent browser QA'}
 out=root.parent.parent/'.local'/'research-render';out.mkdir(parents=True,exist_ok=True)
 (out/'validation.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
 print(json.dumps(result))
