// Dedicated dev-only entry, excluded from the production index build. No hardware capture.
import {useEffect,useState} from "react";
import {createRoot} from "react-dom/client";
import TelehealthVisit from "../src/TelehealthVisit";
import type {VisitDevices} from "../src/TelehealthVisit";
import {visitTokenKey} from "../src/telehealthApi";
import "../src/styles.css";import "../src/workflows.css";import "../src/theme.css";
import {applyTheme,readStoredTheme} from "../src/theme";
type Fixture={name:string;links:{label:string;token:string}[]};
function synthetic(label:string,color:string,audio:boolean):MediaStream {
  const canvas=document.createElement("canvas");canvas.width=640;canvas.height=360;
  const context=canvas.getContext("2d")!;let frames=0;
  const draw=()=>{context.fillStyle=color;context.fillRect(0,0,640,360);context.fillStyle="#fff";context.font="28px sans-serif";context.fillText(label,30,70);context.font="18px sans-serif";context.fillText("Synthetic media · no camera or microphone",30,115);context.fillText(`Frame ${frames++}`,30,155);context.fillRect((frames*5)%580,210,50,50);};draw();
  const timer=setInterval(draw,100);const stream=canvas.captureStream(10);const video=stream.getVideoTracks()[0];const stopVideo=video.stop.bind(video);video.stop=()=>{clearInterval(timer);stopVideo();};
  if(audio){const audioContext=new AudioContext();const oscillator=audioContext.createOscillator();const gain=audioContext.createGain();gain.gain.value=0;const destination=audioContext.createMediaStreamDestination();oscillator.connect(gain).connect(destination);oscillator.start();const track=destination.stream.getAudioTracks()[0];const stop=track.stop.bind(track);let stopped=false;track.stop=()=>{if(stopped)return;stopped=true;oscillator.stop();void audioContext.close();stop();};stream.addTrack(track);}
  return stream;
}
const adapters:VisitDevices={capture:async()=>synthetic("QA CAMERA","#285d51",true),share:async()=>synthetic("QA SHARED SCREEN","#75519c",false),enumerate:async()=>[]};
function Qa(){const[fixture,setFixture]=useState<Fixture|null>(null),[error,setError]=useState("");const[visit]=useState(()=>location.hash.includes("token=")||Boolean(sessionStorage.getItem(visitTokenKey)));
  useEffect(()=>{if(import.meta.env.DEV&&location.port==="5175")fetch("/__nexus_telehealth_fixture").then(r=>{if(!r.ok)throw Error("The isolated QA fixture is not configured.");return r.json();}).then(setFixture).catch(e=>setError(e.message));else setError("This entry is restricted to the dedicated local QA server.");},[]);
  if(!fixture)return <main style={{padding:40}}><h1>Isolated telehealth QA</h1><p>{error||"Checking the isolated fixture…"}</p></main>;
  return <><div style={{padding:10,background:"#fff2cd",color:"#554314",textAlign:"center",fontSize:13}}>ISOLATED QA · generated video and silent audio tracks · no hardware capture · {fixture.name}</div>{visit?<TelehealthVisit devices={adapters}/>:<main style={{padding:40}}><h1>Telehealth transport checks</h1><p>Open each participant in a separate window. Check the generated devices, join, and admit the guests from the host window.</p>{fixture.links.map(link=><p key={link.label}><a className="button" href={`/telehealth-qa.html#token=${link.token}`} target="_blank" rel="noopener noreferrer">Open {link.label}</a></p>)}</main>}</>;
}
applyTheme(readStoredTheme());createRoot(document.getElementById("root")!).render(<Qa/>);
