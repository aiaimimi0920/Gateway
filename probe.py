import websocket,requests,json
j=requests.get('http://localhost:9225/json').json(); u=[x['webSocketDebuggerUrl'] for x in j if x['type']=='page' and x['url'].startswith('https://www.udio.com')][0]
ws=websocket.create_connection(u,timeout=5); ws.send(json.dumps({'id':1,'method':'Runtime.evaluate','params':{'expression':'JSON.stringify({url:location.href,title:document.title,text:document.body.innerText.slice(0,3000),buttons:[...document.querySelectorAll("button")].map(x=>x.innerText).filter(Boolean),inputs:[...document.querySelectorAll("input")].map(x=>({type:x.type,placeholder:x.placeholder,aria:x.getAttribute("aria-label")}))})','returnByValue':True}}));
print(ws.recv())
