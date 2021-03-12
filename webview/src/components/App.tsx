import { Model } from "../model";
import React = require("react");
import { MainView } from "./MainView";
import { hotComponent } from "../utils/hotComponent";

@hotComponent(module)
export class App extends React.Component {
	private readonly model = new Model();

	render() {
		return <MainView model={this.model} />;
	}
}
