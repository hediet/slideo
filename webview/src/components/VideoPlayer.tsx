import * as React from "react";
import videojs from "video.js";

export class VideoPlayer extends React.Component<videojs.PlayerOptions> {
	public player: videojs.Player | undefined;
	private videoNode: HTMLVideoElement | null = null;

	componentDidMount() {
		// instantiate Video.js
		this.player = videojs(
			this.videoNode,
			{ ...this.props, ...{ fill: true } },
			() => {
				console.log("onPlayerReady", this);
			}
		);
	}

	componentWillUnmount() {
		if (this.player) {
			this.player.dispose();
		}
	}

	render() {
		return (
			<div data-vjs-player>
				<video
					ref={(node) => (this.videoNode = node)}
					className="video-js"
				></video>
			</div>
		);
	}
}
