import * as React from "react";
import { Model } from "../model";
import classnames = require("classnames");
import { disposeOnUnmount, observer } from "mobx-react";
import { hotComponent } from "../utils/hotComponent";
import { VideoPlayer } from "./VideoPlayer";
import "video.js/dist/video-js.css";
import { PdfViewer } from "./PdfViewer";
import { autorun, observable } from "mobx";

@hotComponent(module)
@observer
export class MainView extends React.Component<{ model: Model }, {}> {
	@observable.ref
	private player: VideoPlayer | null = null;

	@disposeOnUnmount
	private readonly _updatePlayerSources = autorun(() => {
		if (this.player) {
			console.log("test");
			const videoUrl = this.props.model.videoUrl;
			if (videoUrl) {
				this.player.player!.src([{ src: videoUrl, type: "video/mp4" }]);
				console.log(videoUrl);
			}
		}
	});

	render() {
		const model = this.props.model;

		return (
			<div style={{ height: "100%", display: "flex" }}>
				<div style={{ border: 0, flex: 1 }}>
					{model.matchings && (
						<PdfViewer
							matchings={model.matchings}
							playVideo={(offsetMs, videoHash) => {
								model.currentVideoHash = videoHash;
								if (this.player) {
									this.player.player!.currentTime(
										offsetMs / 1000
									);
									this.player.player!.play();
								}
							}}
							pdfUrl={model.pdfUrl}
						/>
					)}
				</div>
				<div style={{ border: 0, flex: 1 }}>
					{model.videoUrl && (
						<VideoPlayer
							ref={(p) => (this.player = p)}
							controls
							autoplay
							playbackRates={[0.7, 1.0, 1.5, 2.0, 2.5, 3.0]}
						/>
					)}
				</div>
			</div>
		);
	}
}
